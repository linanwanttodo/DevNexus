use crate::utils::crypto::CryptoVault;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::{Arc, Mutex};

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
    crypto: CryptoVault,
}

#[allow(clippy::new_without_default)]
impl PasswordManager {
    pub fn new() -> Self {
        let entries = Arc::new(Mutex::new(Vec::new()));
        let next_id = Arc::new(Mutex::new(1));

        let pm = Self {
            entries: entries.clone(),
            next_id,
            crypto: CryptoVault::new(),
        };

        // 无锁模式：启动即加载已保存条目，不再要求用户设置/输入主密码，
        // 密码管理器 UI 不再显示锁屏界面。
        // 数据仍以 AES-256 加密落盘（密钥来自系统钥匙串/兜底文件）。
        let _ = pm.load_entries();

        pm
    }

    /// 持久化数据文件路径
    fn entries_path() -> std::path::PathBuf {
        crate::utils::data_dir().join("entries.enc")
    }

    /// 保存所有条目到加密文件
    fn save_entries(&self) -> Result<(), String> {
        let path = Self::entries_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let entries = self.entries.lock().map_err(|e| e.to_string())?;
        let json = serde_json::to_string(&*entries).map_err(|e| e.to_string())?;
        let encrypted = self.crypto.encrypt(&json)?;
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

        let encrypted =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read entries: {}", e))?;
        let json = self.crypto.decrypt(&encrypted)?;
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
    let encrypted = state.crypto.encrypt(&password)?;

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

    state.entries.lock().map_err(|e| e.to_string())?.push(entry);

    // 自动持久化
    state.save_entries()?;

    Ok(id)
}

/// 获取所有密码条目（不返回解密后的密码）
#[tauri::command]
pub fn list_passwords(
    state: tauri::State<'_, PasswordManager>,
) -> Result<Vec<PasswordEntry>, String> {
    state
        .entries
        .lock()
        .map(|entries| entries.clone())
        .map_err(|e| e.to_string())
}

/// 获取解密后的密码
#[tauri::command]
pub fn get_password(id: u32, state: tauri::State<'_, PasswordManager>) -> Result<String, String> {
    let entries = state.entries.lock().map_err(|e| e.to_string())?;
    let entry = entries
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| "Password entry not found".to_string())?;

    state.crypto.decrypt(&entry.password_encrypted)
}

/// 删除密码条目
#[tauri::command]
pub fn delete_password(id: u32, state: tauri::State<'_, PasswordManager>) -> Result<(), String> {
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
    let mut entries = state.entries.lock().map_err(|e| e.to_string())?;

    if let Some(entry) = entries.iter_mut().find(|e| e.id == id) {
        entry.name = name;
        entry.username = username;

        if let Some(new_password) = password {
            entry.password_encrypted = state.crypto.encrypt(&new_password)?;
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
    let entries = state.entries.lock().map_err(|e| e.to_string())?;

    let mut csv_content = String::from("name,url,username,password\n");

    for entry in entries.iter() {
        let password = state.crypto.decrypt(&entry.password_encrypted)?;
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
        Ok(format!(
            "Imported {} entries ({} skipped due to errors)",
            count, errors
        ))
    } else {
        Ok(format!("Successfully imported {} entries", count))
    }
}

/// CSV 转义辅助函数
fn escape_csv(field: &str) -> String {
    let escaped = field.replace('"', "\"\"");
    if escaped.contains(',')
        || escaped.contains('"')
        || escaped.contains('\n')
        || escaped.contains('\r')
    {
        format!("\"{}\"", escaped)
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// 用固定密钥构造 PasswordManager，避免测试触碰系统钥匙串/真实数据文件
    fn test_manager() -> PasswordManager {
        PasswordManager {
            entries: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
            crypto: CryptoVault::for_test(),
        }
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let pm = test_manager();
        let secrets: Vec<String> = vec![
            "hunter2".to_string(),
            String::new(),
            "中文密码🔐".to_string(),
            "a".repeat(1024),
            "with\nnewline".to_string(),
        ];
        for s in &secrets {
            let enc = pm.crypto.encrypt(s).expect("encrypt");
            // 密文不应包含明文
            assert!(!enc.contains(s.as_str()) || s.is_empty());
            assert_eq!(pm.crypto.decrypt(&enc).expect("decrypt"), *s);
        }
    }

    #[test]
    fn test_encrypt_produces_randomized_nonce() {
        let pm = test_manager();
        // 相同明文两次加密结果不同（随机 nonce）
        let e1 = pm.crypto.encrypt("same-password").unwrap();
        let e2 = pm.crypto.encrypt("same-password").unwrap();
        assert_ne!(e1, e2);
        // 但都能正确解密
        assert_eq!(pm.crypto.decrypt(&e1).unwrap(), "same-password");
        assert_eq!(pm.crypto.decrypt(&e2).unwrap(), "same-password");
    }

    #[test]
    fn test_decrypt_wrong_key_fails() {
        let pm = test_manager();
        let enc = pm.crypto.encrypt("secret-data").unwrap();
        let wrong = PasswordManager {
            entries: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
            crypto: CryptoVault::for_test_with_key([0x24; 32]),
        };
        assert!(wrong.crypto.decrypt(&enc).is_err());
    }

    #[test]
    fn test_decrypt_invalid_base64_fails() {
        let pm = test_manager();
        assert!(pm.crypto.decrypt("not-base64!!!").is_err());
        assert!(pm.crypto.decrypt("").is_err());
    }

    #[test]
    fn test_decrypt_truncated_data_fails() {
        let pm = test_manager();
        let enc = pm.crypto.encrypt("x").unwrap();
        // 去掉部分字节导致 nonce/密文不完整
        let truncated = &enc[..enc.len() - 4];
        assert!(pm.crypto.decrypt(truncated).is_err());
    }

    #[test]
    fn test_password_entry_serialization() {
        let entry = PasswordEntry {
            id: 1,
            name: "GitHub".to_string(),
            username: "user".to_string(),
            password_encrypted: "enc".to_string(),
            url: Some("https://github.com".to_string()),
            notes: None,
            created_at: "2026-08-15 12:00:00".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"name\":\"GitHub\""));
        assert!(json.contains("\"url\":\"https://github.com\""));
        // 反序列化往返
        let back: PasswordEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, entry.id);
        assert_eq!(back.password_encrypted, "enc");
        assert!(back.notes.is_none());
    }
}
