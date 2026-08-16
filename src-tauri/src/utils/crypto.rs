use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose, Engine as _};
use rand::Rng;
use std::sync::{Arc, Mutex};

/// 共享加密保险库：AES-256-GCM，密钥存 OS keyring（回退 data_dir/password_key.bin）。
/// 与 password_manager 共用同一密钥，保证既有数据可解密。
pub struct CryptoVault {
    key: Arc<Mutex<[u8; 32]>>,
}

#[allow(clippy::new_without_default)]
impl CryptoVault {
    pub fn new() -> Self {
        Self {
            key: Arc::new(Mutex::new(Self::load_or_create_key())),
        }
    }

    #[cfg(test)]
    pub fn for_test() -> Self {
        Self {
            key: Arc::new(Mutex::new([9u8; 32])),
        }
    }

    /// 测试用：显式指定密钥（用于验证错误密钥解密失败）
    #[cfg(test)]
    pub fn for_test_with_key(key: [u8; 32]) -> Self {
        Self {
            key: Arc::new(Mutex::new(key)),
        }
    }

    fn key_file_path() -> std::path::PathBuf {
        crate::utils::data_dir().join("password_key.bin")
    }

    fn load_or_create_key() -> [u8; 32] {
        const SERVICE_NAME: &str = "com.devnexus.app";
        const KEYRING_USER: &str = "encryption-key";

        let entry = keyring::Entry::new(SERVICE_NAME, KEYRING_USER).ok();
        if let Some(ref entry) = entry {
            if let Ok(pw) = entry.get_password() {
                if let Ok(decoded) = general_purpose::STANDARD.decode(&pw) {
                    if decoded.len() == 32 {
                        let mut key = [0u8; 32];
                        key.copy_from_slice(&decoded);
                        Self::try_remove_old_keyfile();
                        return key;
                    }
                }
            }
        }

        if let Some(key) = Self::read_key_file() {
            Self::try_remove_old_keyfile();
            return key;
        }

        if let Some(key) = Self::migrate_from_keyfile(entry.as_ref()) {
            Self::try_remove_old_keyfile();
            return key;
        }

        let mut key = [0u8; 32];
        rand::thread_rng().fill(&mut key);

        let mut persisted = false;
        if let Some(ref entry) = entry {
            let encoded = general_purpose::STANDARD.encode(key);
            match entry.set_password(&encoded) {
                Ok(_) => persisted = true,
                Err(e) => eprintln!("[CryptoVault] Failed to persist key to keyring: {}", e),
            }
        }
        if !persisted {
            persisted = Self::write_key_file(&key);
            if !persisted {
                eprintln!("[CryptoVault] WARNING: unable to persist encryption key (keyring and file both unavailable).");
            }
        }
        if persisted {
            Self::try_remove_old_keyfile();
        }
        key
    }

    fn read_key_file() -> Option<[u8; 32]> {
        let data = std::fs::read(Self::key_file_path()).ok()?;
        if data.len() != 32 {
            return None;
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&data);
        Some(key)
    }

    fn write_key_file(key: &[u8; 32]) -> bool {
        let path = Self::key_file_path();
        if let Some(parent) = path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return false;
            }
        }
        if std::fs::write(&path, key).is_err() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&path, perms);
            }
        }
        true
    }

    fn migrate_from_keyfile(entry: Option<&keyring::Entry>) -> Option<[u8; 32]> {
        // 从 password_manager.rs 原样迁移（旧版 key.bin 迁移逻辑）
        let base = if cfg!(target_os = "macos") {
            std::env::var("HOME")
                .map(|h| std::path::PathBuf::from(h).join("Library/Application Support"))
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
        } else if cfg!(target_os = "windows") {
            std::env::var("APPDATA")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
        } else {
            std::env::var("HOME")
                .map(|h| std::path::PathBuf::from(h).join(".config"))
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
        };
        let key_path = base.join("devnexus").join("key.bin");
        let data = std::fs::read(&key_path).ok()?;
        let key = if data.len() == 48 {
            let mut k = [0u8; 32];
            k.copy_from_slice(&data[16..]);
            k
        } else if data.len() == 32 {
            let mut k = [0u8; 32];
            k.copy_from_slice(&data);
            k
        } else {
            return None;
        };
        if let Some(e) = entry {
            let encoded = general_purpose::STANDARD.encode(key);
            if let Err(err) = e.set_password(&encoded) {
                eprintln!(
                    "[CryptoVault] Failed to persist master password to keyring: {}",
                    err
                );
            }
        }
        Some(key)
    }

    fn try_remove_old_keyfile() {
        let base = if cfg!(target_os = "macos") {
            let Ok(home) = std::env::var("HOME") else {
                return;
            };
            std::path::PathBuf::from(home).join("Library/Application Support")
        } else if cfg!(target_os = "windows") {
            let Ok(appdata) = std::env::var("APPDATA") else {
                return;
            };
            std::path::PathBuf::from(appdata)
        } else {
            let Ok(home) = std::env::var("HOME") else {
                return;
            };
            std::path::PathBuf::from(home).join(".config")
        };
        let _ = std::fs::remove_file(base.join("devnexus").join("key.bin"));
    }

    pub fn encrypt(&self, data: &str) -> Result<String, String> {
        let key = self
            .key
            .lock()
            .map_err(|e| format!("Encryption lock error: {}", e))?;
        let cipher =
            Aes256Gcm::new_from_slice(&key[..]).map_err(|e| format!("Encryption error: {}", e))?;
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, data.as_bytes())
            .map_err(|e| format!("Encryption error: {}", e))?;
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(general_purpose::STANDARD.encode(&combined))
    }

    pub fn decrypt(&self, encrypted_data: &str) -> Result<String, String> {
        let combined = general_purpose::STANDARD
            .decode(encrypted_data)
            .map_err(|e| format!("Decoding error: {}", e))?;
        if combined.len() < 12 {
            return Err("Invalid encrypted data".to_string());
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let key = self
            .key
            .lock()
            .map_err(|e| format!("Decryption lock error: {}", e))?;
        let cipher =
            Aes256Gcm::new_from_slice(&key[..]).map_err(|e| format!("Decryption error: {}", e))?;
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption error: {}", e))?;
        String::from_utf8(plaintext).map_err(|e| format!("UTF-8 error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_vault() -> CryptoVault {
        // 用固定密钥构造，避免触碰 keyring（CI 无 Secret Service）
        CryptoVault {
            key: Arc::new(Mutex::new([7u8; 32])),
        }
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let v = test_vault();
        let plain = "ssh secret \u{4f60}\u{597d} 123";
        let enc = v.encrypt(plain).unwrap();
        assert_ne!(enc, plain);
        assert_eq!(v.decrypt(&enc).unwrap(), plain);
    }

    #[test]
    fn test_encrypt_produces_randomized_nonce() {
        let v = test_vault();
        let a = v.encrypt("same").unwrap();
        let b = v.encrypt("same").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_decrypt_invalid_base64_fails() {
        let v = test_vault();
        assert!(v.decrypt("!!!not-base64!!!").is_err());
    }

    #[test]
    fn test_decrypt_truncated_data_fails() {
        let v = test_vault();
        assert!(v.decrypt("AAAA").is_err());
    }
}
