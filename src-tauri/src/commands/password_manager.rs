use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use base64::{Engine as _, engine::general_purpose};
use rand::Rng;
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct PasswordEntry {
    pub id: u32,
    pub name: String,
    pub username: String,
    pub password_encrypted: String,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

type VerifierState = Option<(Vec<u8>, Vec<u8>)>;

pub struct PasswordManager {
    pub entries: Arc<Mutex<Vec<PasswordEntry>>>,
    pub next_id: Arc<Mutex<u32>>,
    encryption_key: [u8; 32], // AES-256 key
    pub locked: Arc<Mutex<bool>>,
    password_verifier: Arc<Mutex<VerifierState>>, // (salt, hash) for master password verification
}

impl PasswordManager {
    pub fn new() -> Self {
        let key = Self::load_or_create_key();

        let entries = Arc::new(Mutex::new(Vec::new()));
        let next_id = Arc::new(Mutex::new(1));

        // 不自动加载条目，等待 unlock
        let mut pm = Self {
            entries: entries.clone(),
            next_id,
            encryption_key: key,
            locked: Arc::new(Mutex::new(true)),
            password_verifier: Arc::new(Mutex::new(None)),
        };

        // 尝试加载已保存的密码验证器
        let _ = pm.load_verifier();
        // 如果没有设置主密码，则保持解锁状态用于首次设置
        let verifier = pm.password_verifier.lock().unwrap();
        let has_verifier = verifier.is_some();
        drop(verifier);

        if !has_verifier {
            // 没有设置主密码时自动解锁（首次使用）
            *pm.locked.lock().unwrap() = false;
            let _ = pm.load_entries();
        }

        pm
    }

    /// 持久化数据文件路径
    fn entries_path() -> std::path::PathBuf {
        let base = Self::data_dir();
        base.join("entries.enc")
    }

    fn data_dir() -> std::path::PathBuf {
        if cfg!(target_os = "macos") {
            std::env::var("HOME")
                .map(|h| std::path::PathBuf::from(h).join("Library/Application Support/devnexus"))
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
        } else if cfg!(target_os = "windows") {
            std::env::var("APPDATA")
                .map(|h| std::path::PathBuf::from(h).join("devnexus"))
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
        } else {
            std::env::var("XDG_DATA_HOME")
                .map(|h| std::path::PathBuf::from(h).join("devnexus"))
                .or_else(|_| {
                    std::env::var("HOME")
                        .map(|h| std::path::PathBuf::from(h).join(".local/share/devnexus"))
                })
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
        }
    }

    /// 保存所有条目到加密文件
    fn save_entries(&self) -> Result<(), String> {
        let path = Self::entries_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let entries = self.entries.lock().map_err(|e| e.to_string())?;
        let json = serde_json::to_string(&*entries).map_err(|e| e.to_string())?;
        let encrypted = self.encrypt(&json)?;
        fs::write(&path, &encrypted).map_err(|e| format!("Failed to save entries: {}", e))?;

        // 设置文件权限 0600
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }

        Ok(())
    }

    /// 从加密文件加载条目
    fn load_entries(&self) -> Result<(), String> {
        let path = Self::entries_path();
        if !path.exists() {
            return Ok(()); // 首次运行，无文件
        }

        let encrypted = fs::read_to_string(&path).map_err(|e| format!("Failed to read entries: {}", e))?;
        let json = self.decrypt(&encrypted)?;
        let loaded_entries: Vec<PasswordEntry> =
            serde_json::from_str(&json).map_err(|e| format!("Failed to parse entries: {}", e))?;

        let mut entries = self.entries.lock().map_err(|e| e.to_string())?;
        *entries = loaded_entries;

        // 恢复 next_id
        let max_id = entries.iter().map(|e| e.id).max().unwrap_or(0);
        let mut next_id = self.next_id.lock().map_err(|e| e.to_string())?;
        *next_id = max_id + 1;

        Ok(())
    }

    /// 从系统钥匙串（keyring）加载或创建加密密钥
    /// 使用 OS 原生安全存储（macOS Keychain / Linux Secret Service / Windows Credential Manager）
    /// 替代旧版 flat file 方案，避免密钥以明文形式暴露在文件系统中
    fn load_or_create_key() -> [u8; 32] {
        const SERVICE_NAME: &str = "com.devnexus.app";
        const KEYRING_USER: &str = "encryption-key";

        // 1. 优先从系统钥匙串读取
        let entry = keyring::Entry::new(SERVICE_NAME, KEYRING_USER).ok();
        if let Some(ref entry) = entry {
            if let Ok(pw) = entry.get_password() {
                if let Ok(decoded) = general_purpose::STANDARD.decode(&pw) {
                    if decoded.len() == 32 {
                        let mut key = [0u8; 32];
                        key.copy_from_slice(&decoded);
                        // 迁移后清理旧版 key.bin
                        Self::try_remove_old_keyfile();
                        return key;
                    }
                }
            }
        }

        // 2. 向后兼容：尝试从旧版 key.bin 迁移
        if let Some(key) = Self::migrate_from_keyfile(entry.as_ref()) {
            Self::try_remove_old_keyfile();
            return key;
        }

        // 3. 生成新密钥并存入钥匙串
        let mut key = [0u8; 32];
        rand::thread_rng().fill(&mut key);
        let encoded = general_purpose::STANDARD.encode(key);

        if let Some(ref entry) = entry {
            let _ = entry.set_password(&encoded);
        }

        Self::try_remove_old_keyfile();
        key
    }

    /// 从旧版 key.bin 迁移密钥到钥匙串
    fn migrate_from_keyfile(entry: Option<&keyring::Entry>) -> Option<[u8; 32]> {
        let key_path = {
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
            base.join("devnexus").join("key.bin")
        };

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

        // 写入钥匙串
        if let Some(e) = entry {
            let encoded = general_purpose::STANDARD.encode(key);
            let _ = e.set_password(&encoded);
        }

        Some(key)
    }

    fn try_remove_old_keyfile() {
        let base = if cfg!(target_os = "macos") {
            let Ok(home) = std::env::var("HOME") else { return };
            std::path::PathBuf::from(home).join("Library/Application Support")
        } else if cfg!(target_os = "windows") {
            let Ok(appdata) = std::env::var("APPDATA") else { return };
            std::path::PathBuf::from(appdata)
        } else {
            let Ok(home) = std::env::var("HOME") else { return };
            std::path::PathBuf::from(home).join(".config")
        };
        let key_path = base.join("devnexus").join("key.bin");
        let _ = std::fs::remove_file(key_path);
    }

    /// 加密数据
    fn encrypt(&self, data: &str) -> Result<String, String> {
        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|e| format!("Encryption error: {}", e))?;
        
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = cipher.encrypt(nonce, data.as_bytes())
            .map_err(|e| format!("Encryption error: {}", e))?;
        
        // 将 nonce 和 ciphertext 组合并 base64 编码
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        
        Ok(general_purpose::STANDARD.encode(&combined))
    }

    /// 解密数据
    fn decrypt(&self, encrypted_data: &str) -> Result<String, String> {
        let combined = general_purpose::STANDARD.decode(encrypted_data)
            .map_err(|e| format!("Decoding error: {}", e))?;
        
        if combined.len() < 12 {
            return Err("Invalid encrypted data".to_string());
        }
        
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        
        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|e| format!("Decryption error: {}", e))?;
        
        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption error: {}", e))?;
        
        String::from_utf8(plaintext)
            .map_err(|e| format!("UTF-8 error: {}", e))
    }

    /// 主密码验证器文件路径
    fn verifier_path() -> std::path::PathBuf {
        let base = Self::data_dir();
        base.join("master.verifier")
    }

    /// 保存主密码验证器（salt + hash）
    fn save_verifier(&self, salt: Vec<u8>, hash: Vec<u8>) -> Result<(), String> {
        let path = Self::verifier_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // 格式: salt(16 bytes) + hash(32 bytes)
        let mut data = salt;
        data.extend_from_slice(&hash);
        fs::write(&path, data).map_err(|e| format!("Failed to save verifier: {}", e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// 加载主密码验证器
    fn load_verifier(&mut self) -> Result<(), String> {
        let path = Self::verifier_path();
        if !path.exists() {
            return Ok(());
        }
        let data = fs::read(&path).map_err(|e| format!("Failed to read verifier: {}", e))?;
        if data.len() != 48 {
            return Err("Invalid verifier file".to_string());
        }
        let salt = data[..16].to_vec();
        let hash = data[16..48].to_vec();
        *self.password_verifier.lock().map_err(|e| e.to_string())? = Some((salt, hash));
        Ok(())
    }

    /// 检查是否锁定，如果锁定则返回错误
    fn check_locked(&self) -> Result<(), String> {
        if *self.locked.lock().map_err(|e| e.to_string())? {
            Err("Password manager is locked. Please unlock first.".to_string())
        } else {
            Ok(())
        }
    }
}

/// 查询密码管理器是否锁定
#[tauri::command]
pub fn is_locked(state: tauri::State<'_, PasswordManager>) -> bool {
    state.locked.lock().map(|l| *l).unwrap_or(true)
}

/// 设置主密码（首次使用）
#[tauri::command]
pub fn set_master_password(
    master_password: String,
    state: tauri::State<'_, PasswordManager>,
) -> Result<(), String> {
    // 检查是否已设置主密码
    let verifier = state.password_verifier.lock().map_err(|e| e.to_string())?;
    if verifier.is_some() {
        return Err("Master password already set. Use unlock to access.".to_string());
    }
    drop(verifier);

    // 生成 salt 并使用 PBKDF2 哈希密码
    let salt = generate_salt();
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;
    let mut hash = [0u8; 32];
    pbkdf2_hmac::<Sha256>(master_password.as_bytes(), &salt, PBKDF2_ITERATIONS, &mut hash);

    state.save_verifier(salt.to_vec(), hash.to_vec())?;

    // 解锁状态
    *state.locked.lock().map_err(|e| e.to_string())? = false;
    // 加载已保存的条目
    let _ = state.load_entries();

    Ok(())
}

/// 用主密码解锁密码管理器
#[tauri::command]
pub fn unlock(
    master_password: String,
    state: tauri::State<'_, PasswordManager>,
) -> Result<bool, String> {
    let verifier = state.password_verifier.lock().map_err(|e| e.to_string())?;
    let (salt, stored_hash) = verifier.as_ref()
        .ok_or_else(|| "No master password set. Please set one first.".to_string())?;
    let salt = salt.clone();
    let stored_hash = stored_hash.clone();
    drop(verifier);

    // 验证密码
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;
    let mut hash = [0u8; 32];
    pbkdf2_hmac::<Sha256>(master_password.as_bytes(), &salt, PBKDF2_ITERATIONS, &mut hash);

    if hash.as_slice() != stored_hash.as_slice() {
        return Ok(false);
    }

    // 解锁并加载条目
    *state.locked.lock().map_err(|e| e.to_string())? = false;
    let _ = state.load_entries();

    Ok(true)
}

/// 锁定密码管理器
#[tauri::command]
pub fn lock(state: tauri::State<'_, PasswordManager>) -> Result<(), String> {
    // 清空内存中的条目
    state.entries.lock().map_err(|e| e.to_string())?.clear();
    *state.locked.lock().map_err(|e| e.to_string())? = true;
    Ok(())
}

/// 是否已设置主密码
#[tauri::command]
pub fn has_master_password(state: tauri::State<'_, PasswordManager>) -> bool {
    state.password_verifier.lock().map(|v| v.is_some()).unwrap_or(false)
}

/// 添加密码条目
#[tauri::command]
pub fn add_password(
    name: String,
    username: String,
    password: String,
    url: Option<String>,
    notes: Option<String>,
    state: tauri::State<'_, PasswordManager>,
) -> Result<u32, String> {
    state.check_locked()?;
    let encrypted = state.encrypt(&password)?;
    
    let mut next_id = state.next_id.lock().map_err(|e| e.to_string())?;
    let id = *next_id;
    *next_id += 1;
    drop(next_id);

    let entry = PasswordEntry {
        id,
        name,
        username,
        password_encrypted: encrypted,
        url,
        notes,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    state.entries.lock()
        .map_err(|e| e.to_string())?
        .push(entry);

    // 自动持久化
    state.save_entries()?;

    Ok(id)
}

/// 获取所有密码条目（不返回解密后的密码）
#[tauri::command]
pub fn list_passwords(state: tauri::State<'_, PasswordManager>) -> Vec<PasswordEntry> {
    if state.check_locked().is_err() {
        return Vec::new();
    }
    state.entries.lock()
        .map(|entries| entries.clone())
        .unwrap_or_else(|_| Vec::new())
}

/// 获取解密后的密码
#[tauri::command]
pub fn get_password(id: u32, state: tauri::State<'_, PasswordManager>) -> Result<String, String> {
    state.check_locked()?;
    let entries = state.entries.lock().map_err(|e| e.to_string())?;
    let entry = entries.iter().find(|e| e.id == id)
        .ok_or_else(|| "Password entry not found".to_string())?;
    
    state.decrypt(&entry.password_encrypted)
}

/// 删除密码条目
#[tauri::command]
pub fn delete_password(id: u32, state: tauri::State<'_, PasswordManager>) -> Result<(), String> {
    state.check_locked()?;
    let mut entries = state.entries.lock().map_err(|e| e.to_string())?;
    entries.retain(|e| e.id != id);
    drop(entries);
    state.save_entries()?;
    Ok(())
}

/// 更新密码条目
#[tauri::command]
pub fn update_password(
    id: u32,
    name: String,
    username: String,
    password: Option<String>,
    url: Option<String>,
    notes: Option<String>,
    state: tauri::State<'_, PasswordManager>,
) -> Result<(), String> {
    state.check_locked()?;
    let mut entries = state.entries.lock().map_err(|e| e.to_string())?;
    
    if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
        entry.name = name;
        entry.username = username;
        
        if let Some(new_password) = password {
            entry.password_encrypted = state.encrypt(&new_password)?;
        }
        
        entry.url = url;
        entry.notes = notes;
        
        drop(entries);
        state.save_entries()?;
        Ok(())
    } else {
        Err("Password entry not found".to_string())
    }
}

/// 导出为 Chrome CSV 格式
#[tauri::command]
pub fn export_chrome_csv(state: tauri::State<'_, PasswordManager>) -> Result<String, String> {
    state.check_locked()?;
    let entries = state.entries.lock().map_err(|e| e.to_string())?;
    
    let mut csv_content = String::from("name,url,username,password\n");
    
    for entry in entries.iter() {
        let password = state.decrypt(&entry.password_encrypted)?;
        let url = entry.url.as_deref().unwrap_or("");
        
        // CSV 转义
        csv_content.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{}\"\n",
            escape_csv(&entry.name),
            escape_csv(url),
            escape_csv(&entry.username),
            escape_csv(&password)
        ));
    }
    
    Ok(csv_content)
}

/// 从 Chrome CSV 导入
#[tauri::command]
pub fn import_chrome_csv(
    csv_content: String,
    state: tauri::State<'_, PasswordManager>,
) -> Result<String, String> {
    state.check_locked()?;
    let mut count = 0;
    let mut errors = 0;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv_content.as_bytes());
    
    for result in reader.records() {
        let record = match result {
            Ok(r) => r,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        
        if record.len() >= 4 {
            let name = record[0].to_string();
            let url = record[1].to_string();
            let username = record[2].to_string();
            let password = record[3].to_string();
            
            match add_password(
                name,
                username,
                password,
                if url.is_empty() { None } else { Some(url) },
                None,
                state.clone(),
            ) {
                Ok(_) => count += 1,
                Err(_) => errors += 1,
            }
        }
    }
    
    if errors > 0 {
        Ok(format!("Imported {} entries ({} skipped due to errors)", count, errors))
    } else {
        Ok(format!("Successfully imported {} entries", count))
    }
}

/// 保存到文件（加密，使用 PBKDF2 + AES-256-GCM）
///
/// 文件格式: Base64( salt(16) || iterations(4 LE) || nonce(12) || ciphertext )
#[tauri::command]
pub fn save_to_file(
    file_path: String,
    master_password: String,
    state: tauri::State<'_, PasswordManager>,
) -> Result<(), String> {
    let entries = state.entries.lock().map_err(|e| e.to_string())?;
    let json = serde_json::to_string(&*entries).map_err(|e| e.to_string())?;

    let salt = generate_salt();
    let key = derive_key(&master_password, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;

    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, json.as_bytes())
        .map_err(|e| format!("Encryption error: {}", e))?;

    // 构建二进制包: salt(16) + iterations(4) + nonce(12) + ciphertext
    let mut combined = salt.to_vec();
    combined.extend_from_slice(&PBKDF2_ITERATIONS.to_le_bytes());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    let encoded = general_purpose::STANDARD.encode(&combined);
    fs::write(&file_path, encoded).map_err(|e| format!("File write error: {}", e))?;

    Ok(())
}

/// 从文件加载（解密，自动解析 PBKDF2 参数）
#[tauri::command]
pub fn load_from_file(
    file_path: String,
    master_password: String,
    state: tauri::State<'_, PasswordManager>,
) -> Result<u32, String> {
    let encoded =
        fs::read_to_string(&file_path).map_err(|e| format!("File read error: {}", e))?;

    let combined = general_purpose::STANDARD
        .decode(&encoded)
        .map_err(|e| format!("Decode error: {}", e))?;

    // 最小长度: salt(16) + iterations(4) + nonce(12) = 32
    if combined.len() < 32 {
        return Err("Invalid or corrupted file format".to_string());
    }

    let salt: [u8; 16] = combined[..16]
        .try_into()
        .map_err(|_| "Invalid salt".to_string())?;

    let iterations = u32::from_le_bytes(
        combined[16..20]
            .try_into()
            .map_err(|_| "Invalid iterations".to_string())?,
    );

    let nonce_bytes: [u8; 12] = combined[20..32]
        .try_into()
        .map_err(|_| "Invalid nonce".to_string())?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = &combined[32..];

    // 使用存储的迭代次数派生密钥（兼容未来迭代次数变更）
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
        master_password.as_bytes(),
        &salt,
        iterations,
        &mut key,
    );

    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed (wrong password?): {}", e))?;

    let json = String::from_utf8(plaintext).map_err(|e| e.to_string())?;

    let entries: Vec<PasswordEntry> =
        serde_json::from_str(&json).map_err(|e| format!("JSON parse error: {}", e))?;

    let count = entries.len() as u32;
    let mut stored_entries = state.entries.lock().map_err(|e| e.to_string())?;
    *stored_entries = entries;

    Ok(count)
}

/// CSV 转义辅助函数
fn escape_csv(field: &str) -> String {
    let escaped = field.replace('"', "\"\"");
    if escaped.contains(',') || escaped.contains('"') || escaped.contains('\n') || escaped.contains('\r') {
        format!("\"{}\"", escaped)
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_salt_length() {
        let salt = generate_salt();
        assert_eq!(salt.len(), 16);
        // 两次调用应该产生不同的 salt
        let salt2 = generate_salt();
        assert_ne!(salt, salt2);
    }

    #[test]
    fn test_derive_key_deterministic() {
        let salt = b"0123456789abcdef";
        let key1 = derive_key("my_password", salt);
        let key2 = derive_key("my_password", salt);
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32);
    }

    #[test]
    fn test_derive_key_different_passwords() {
        let salt = b"0123456789abcdef";
        let key1 = derive_key("password1", salt);
        let key2 = derive_key("password2", salt);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_derive_key_different_salts() {
        let key1 = derive_key("password", b"1111111111111111");
        let key2 = derive_key("password", b"2222222222222222");
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_escape_csv_no_special() {
        assert_eq!(escape_csv("hello"), "hello");
    }

    #[test]
    fn test_escape_csv_with_comma() {
        assert_eq!(escape_csv("hello,world"), "\"hello,world\"");
    }

    #[test]
    fn test_escape_csv_with_quotes() {
        assert_eq!(escape_csv("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_escape_csv_with_newline() {
        assert_eq!(escape_csv("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_escape_csv_empty() {
        assert_eq!(escape_csv(""), "");
    }
}

// PBKDF2 迭代次数（100,000 次，平衡安全与性能）
const PBKDF2_ITERATIONS: u32 = 100_000;

/// 使用 PBKDF2-HMAC-SHA256 从密码派生 32 字节密钥
fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;

    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

/// 生成随机 salt
fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    rand::Rng::fill(&mut rand::thread_rng(), &mut salt);
    salt
}
