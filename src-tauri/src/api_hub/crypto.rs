//! API Key 静态加密（AES-256-GCM）
//!
//! 存储边界加密：SQLite 中只保存 `enc1:{base64(nonce(12) || ciphertext)}`，
//! 内存中的 `Provider.api_key` 保持明文（forwarder 需要真实 key 调用上游）。
//!
//! 密钥获取回退链：
//!   1. OS keyring（service="devnexus", user="api-hub-key"）
//!   2. data_dir/api_hub_key.bin（0600 权限，文件兜底）
//!   3. 明文降级（enabled=false + eprintln 警告），确保 headless/CI 环境不炸

use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose, Engine as _};
use rand::Rng;

const KEYRING_SERVICE: &str = "devnexus";
const KEYRING_USER: &str = "api-hub-key";
const KEY_FILE: &str = "api_hub_key.bin";
const ENC_PREFIX: &str = "enc1:";

/// 持有 32 字节 AES-256 密钥 + 标记是否启用加密
pub struct ApiKeyCipher {
    key: [u8; 32],
    enabled: bool,
}

impl ApiKeyCipher {
    /// 是否启用加密（false 表示明文降级，前端需提示风险）
    pub fn is_encrypted(&self) -> bool {
        self.enabled
    }

    /// 从 OS keyring 读取或创建密钥（service="devnexus", user="api-hub-key"）。
    ///
    /// 回退链：keyring → `data_dir/api_hub_key.bin`（0600 权限，文件兜底）→ 明文降级
    /// （enabled=false + eprintln 警告）。保证无桌面密钥环（headless/CI）环境不 panic。
    pub fn load_or_create(data_dir: &std::path::Path) -> Self {
        let key_path = data_dir.join(KEY_FILE);
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).ok();

        // 1) 优先从 keyring 读取
        if let Some(ref entry) = entry {
            if let Ok(pw) = entry.get_password() {
                if let Some(key) = decode_key(&pw) {
                    return ApiKeyCipher { key, enabled: true };
                }
            }
        }

        // 2) 回退：从 data_dir 文件读取
        if let Ok(raw) = std::fs::read(&key_path) {
            if raw.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&raw);
                return ApiKeyCipher { key, enabled: true };
            }
        }

        // 3) 生成新密钥并尝试持久化
        let mut key = [0u8; 32];
        rand::thread_rng().fill(&mut key);

        if let Some(ref entry) = entry {
            if entry
                .set_password(&general_purpose::STANDARD.encode(key))
                .is_ok()
            {
                return ApiKeyCipher { key, enabled: true };
            }
        }

        if write_key_file(&key_path, &key) {
            return ApiKeyCipher { key, enabled: true };
        }

        // 4) 明文降级（keyring 与文件均不可用）
        eprintln!(
            "[API Hub] WARNING: unable to persist api_key encryption key (keyring and {} both unavailable). \
             Falling back to PLAINTEXT storage for API keys.",
            key_path.display()
        );
        ApiKeyCipher {
            key,
            enabled: false,
        }
    }

    /// 测试/嵌入场景：以固定密钥构造，不访问 keyring/文件系统
    #[doc(hidden)]
    pub fn from_key(key: [u8; 32], enabled: bool) -> Self {
        ApiKeyCipher { key, enabled }
    }

    /// 加密：格式 `enc1:{base64(nonce(12) || ciphertext)}`，AES-256-GCM，nonce 每次随机。
    /// enabled=false（明文降级）时直接返回原文。
    pub fn encrypt(&self, plaintext: &str) -> Result<String, String> {
        if !self.enabled {
            return Ok(plaintext.to_string());
        }
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|e| format!("Encryption init error: {}", e))?;
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| format!("Encryption error: {}", e))?;
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(format!(
            "{}{}",
            ENC_PREFIX,
            general_purpose::STANDARD.encode(&combined)
        ))
    }

    /// 解密：以 `enc1:` 前缀识别加密值；无前缀视为旧明文（兼容迁移），原样返回。
    /// enabled=false（明文降级）时直接返回原文。
    /// 解密失败（篡改 / 密钥不匹配）时返回空串并 eprintln 警告（fail-closed）。
    pub fn decrypt(&self, stored: &str) -> String {
        if !self.enabled {
            return stored.to_string();
        }
        let payload = match stored.strip_prefix(ENC_PREFIX) {
            Some(p) => p,
            None => return stored.to_string(), // 旧明文，兼容迁移
        };
        let combined = match general_purpose::STANDARD.decode(payload) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[API Hub] Failed to base64-decode encrypted api_key: {}", e);
                return String::new();
            }
        };
        if combined.len() < 12 {
            eprintln!("[API Hub] Invalid encrypted api_key payload (too short)");
            return String::new();
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = match Aes256Gcm::new_from_slice(&self.key) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[API Hub] Invalid encryption key: {}", e);
                return String::new();
            }
        };
        match cipher.decrypt(nonce, ciphertext) {
            Ok(pt) => String::from_utf8_lossy(&pt).to_string(),
            Err(e) => {
                eprintln!(
                    "[API Hub] Failed to decrypt api_key (corrupted or wrong key): {}",
                    e
                );
                String::new()
            }
        }
    }
}

/// 从 base64 字符串还原 32 字节密钥；失败返回 None
fn decode_key(encoded: &str) -> Option<[u8; 32]> {
    let decoded = general_purpose::STANDARD.decode(encoded).ok()?;
    if decoded.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    Some(key)
}

/// 写密钥文件（Unix 上设置 0600 权限）；失败返回 false
fn write_key_file(path: &std::path::Path, key: &[u8; 32]) -> bool {
    if let Some(parent) = path.parent() {
        if !parent.exists() && std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    if std::fs::write(path, key).is_err() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_cipher() -> ApiKeyCipher {
        ApiKeyCipher::from_key([7u8; 32], true)
    }

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let c = enabled_cipher();
        let enc = c.encrypt("sk-test-12345").unwrap();
        assert!(
            enc.starts_with(ENC_PREFIX),
            "encrypted value must carry prefix"
        );
        assert_eq!(c.decrypt(&enc), "sk-test-12345");
    }

    #[test]
    fn decrypt_without_prefix_returns_plaintext() {
        let c = enabled_cipher();
        assert_eq!(c.decrypt("legacy-plain-key"), "legacy-plain-key");
        assert_eq!(c.decrypt(""), "");
    }

    #[test]
    fn tampered_ciphertext_fails_closed() {
        let c = enabled_cipher();
        let enc = c.encrypt("secret").unwrap();
        let payload = enc.strip_prefix(ENC_PREFIX).unwrap();
        let mut combined = general_purpose::STANDARD.decode(payload).unwrap();
        let last = combined.len() - 1;
        combined[last] ^= 0x01; // 篡改一个字节 → GCM 认证失败
        let tampered = format!(
            "{}{}",
            ENC_PREFIX,
            general_purpose::STANDARD.encode(&combined)
        );
        assert_eq!(
            c.decrypt(&tampered),
            "",
            "tampered ciphertext must fail closed"
        );
    }

    #[test]
    fn disabled_mode_passthrough() {
        let c = ApiKeyCipher::from_key([0u8; 32], false);
        assert_eq!(c.encrypt("plain-secret").unwrap(), "plain-secret");
        assert_eq!(c.decrypt("whatever"), "whatever");
    }

    #[test]
    fn nonce_is_random_per_encryption() {
        let c = enabled_cipher();
        let a = c.encrypt("same").unwrap();
        let b = c.encrypt("same").unwrap();
        assert_ne!(a, b, "each encryption must use a fresh random nonce");
    }
}
