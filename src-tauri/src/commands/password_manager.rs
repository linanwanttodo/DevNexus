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

pub struct PasswordManager {
    pub entries: Arc<Mutex<Vec<PasswordEntry>>>,
    pub next_id: Arc<Mutex<u32>>,
    encryption_key: [u8; 32], // AES-256 key
}

impl PasswordManager {
    pub fn new() -> Self {
        let key = Self::load_or_create_key();

        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
            encryption_key: key,
        }
    }

    /// 从持久化文件加载密钥，不存在则生成并保存
    fn load_or_create_key() -> [u8; 32] {
        let key_path = Self::key_path();

        // 尝试从文件加载
        if let Ok(data) = std::fs::read(&key_path) {
            if data.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&data);
                return key;
            }
        }

        // 生成新密钥：基于机器 hostname + 随机盐
        let host = sysinfo::System::host_name()
            .unwrap_or_else(|| "devnexus".to_string());

        let mut salt = [0u8; 16];
        rand::thread_rng().fill(&mut salt);

        let mut key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
            host.as_bytes(),
            &salt,
            PBKDF2_ITERATIONS,
            &mut key,
        );

        // 保存到文件（仅用户可读）
        if let Some(parent) = key_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&key_path, &key).is_err() {
            // 写文件失败退化为随机密钥（本次会话可用）
            rand::thread_rng().fill(&mut key);
        } else {
            // 设置文件权限 0600
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
            }
        }

        key
    }

    fn key_path() -> std::path::PathBuf {
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

    Ok(id)
}

/// 获取所有密码条目（不返回解密后的密码）
#[tauri::command]
pub fn list_passwords(state: tauri::State<'_, PasswordManager>) -> Vec<PasswordEntry> {
    state.entries.lock()
        .map(|entries| entries.clone())
        .unwrap_or_else(|_| Vec::new())
}

/// 获取解密后的密码
#[tauri::command]
pub fn get_password(id: u32, state: tauri::State<'_, PasswordManager>) -> Result<String, String> {
    let entries = state.entries.lock().map_err(|e| e.to_string())?;
    let entry = entries.iter().find(|e| e.id == id)
        .ok_or_else(|| "Password entry not found".to_string())?;
    
    state.decrypt(&entry.password_encrypted)
}

/// 删除密码条目
#[tauri::command]
pub fn delete_password(id: u32, state: tauri::State<'_, PasswordManager>) -> Result<(), String> {
    let mut entries = state.entries.lock().map_err(|e| e.to_string())?;
    entries.retain(|e| e.id != id);
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
    let mut entries = state.entries.lock().map_err(|e| e.to_string())?;
    
    if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
        entry.name = name;
        entry.username = username;
        
        if let Some(new_password) = password {
            entry.password_encrypted = state.encrypt(&new_password)?;
        }
        
        entry.url = url;
        entry.notes = notes;
        
        Ok(())
    } else {
        Err("Password entry not found".to_string())
    }
}

/// 导出为 Chrome CSV 格式
#[tauri::command]
pub fn export_chrome_csv(state: tauri::State<'_, PasswordManager>) -> Result<String, String> {
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
