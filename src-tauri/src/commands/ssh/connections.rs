use crate::utils::crypto::CryptoVault;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone)]
pub struct SshConnection {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub encrypted_secret: String,
    pub key_passphrase_encrypted: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SshConnectionInfo {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct SshConnectionInput {
    pub id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub secret: String,
    pub key_passphrase: Option<String>,
}

pub struct SshStore {
    pub vault: CryptoVault,
    pub conns: Arc<Mutex<Vec<SshConnection>>>,
    pub next_id: Arc<Mutex<u64>>,
}

#[allow(clippy::new_without_default)]
impl SshStore {
    pub fn new() -> Self {
        let store = Self {
            vault: CryptoVault::new(),
            conns: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        };
        let _ = store.load();
        store
    }

    fn load(&self) -> Result<(), String> {
        let path = Self::conns_path();
        if !path.exists() {
            return Ok(());
        }
        let data = std::fs::read(&path).map_err(|e| format!("read: {}", e))?;
        let list: Vec<SshConnection> =
            serde_json::from_slice(&data).map_err(|e| format!("parse: {}", e))?;
        let mut conns = self.conns.lock().map_err(|e| e.to_string())?;
        *conns = list;
        let max = conns
            .iter()
            .filter_map(|c| c.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        let mut nid = self.next_id.lock().map_err(|e| e.to_string())?;
        *nid = max + 1;
        Ok(())
    }

    fn save(&self) -> Result<(), String> {
        let path = Self::conns_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conns = self.conns.lock().map_err(|e| e.to_string())?;
        let data = serde_json::to_vec(&*conns).map_err(|e| e.to_string())?;
        std::fs::write(&path, &data).map_err(|e| format!("write: {}", e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    fn conns_path() -> std::path::PathBuf {
        crate::utils::data_dir().join("ssh_connections.json")
    }

    fn encrypt_secret(
        &self,
        input: &SshConnectionInput,
    ) -> Result<(String, Option<String>), String> {
        let enc = self.vault.encrypt(&input.secret)?;
        let pass = match &input.key_passphrase {
            Some(p) if !p.is_empty() => Some(self.vault.encrypt(p)?),
            _ => None,
        };
        Ok((enc, pass))
    }

    pub fn to_info(c: &SshConnection) -> SshConnectionInfo {
        SshConnectionInfo {
            id: c.id.clone(),
            name: c.name.clone(),
            host: c.host.clone(),
            port: c.port,
            username: c.username.clone(),
            auth_type: c.auth_type.clone(),
        }
    }

    pub fn find(&self, id: &str) -> Option<SshConnection> {
        self.conns.lock().ok()?.iter().find(|c| c.id == id).cloned()
    }

    /// 供 session.rs 使用：解密 secret（密码或私钥 PEM 文本）
    pub fn decrypt_secret(&self, conn: &SshConnection) -> Result<String, String> {
        self.vault.decrypt(&conn.encrypted_secret)
    }

    pub fn decrypt_passphrase(&self, conn: &SshConnection) -> Result<Option<String>, String> {
        match &conn.key_passphrase_encrypted {
            Some(enc) => Ok(Some(self.vault.decrypt(enc)?)),
            None => Ok(None),
        }
    }
}

impl From<SshConnection> for SshConnectionInfo {
    fn from(c: SshConnection) -> Self {
        SshStore::to_info(&c)
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn ssh_list_connections(
    state: tauri::State<SshStore>,
) -> Result<Vec<SshConnectionInfo>, String> {
    let conns = state.conns.lock().map_err(|e| e.to_string())?;
    Ok(conns.iter().map(SshStore::to_info).collect())
}

#[tauri::command]
pub fn ssh_save_connection(
    state: tauri::State<SshStore>,
    connection: SshConnectionInput,
) -> Result<SshConnectionInfo, String> {
    let (enc, pass) = state.encrypt_secret(&connection)?;
    let ts = now_ts();
    let mut conns = state.conns.lock().map_err(|e| e.to_string())?;
    let conn = match &connection.id {
        Some(id) if conns.iter().any(|c| &c.id == id) => {
            let c = conns.iter_mut().find(|c| &c.id == id).unwrap();
            c.name = connection.name;
            c.host = connection.host;
            c.port = connection.port;
            c.username = connection.username;
            c.auth_type = connection.auth_type;
            c.encrypted_secret = enc;
            c.key_passphrase_encrypted = pass;
            c.updated_at = ts;
            c.clone()
        }
        _ => {
            let mut nid = state.next_id.lock().map_err(|e| e.to_string())?;
            let id = nid.to_string();
            *nid += 1;
            let c = SshConnection {
                id: id.clone(),
                name: connection.name,
                host: connection.host,
                port: connection.port,
                username: connection.username,
                auth_type: connection.auth_type,
                encrypted_secret: enc,
                key_passphrase_encrypted: pass,
                created_at: ts,
                updated_at: ts,
            };
            conns.push(c.clone());
            c
        }
    };
    drop(conns);
    state.save()?;
    Ok(SshStore::to_info(&conn))
}

#[tauri::command]
pub fn ssh_delete_connection(state: tauri::State<SshStore>, id: String) -> Result<(), String> {
    let mut conns = state.conns.lock().map_err(|e| e.to_string())?;
    conns.retain(|c| c.id != id);
    drop(conns);
    state.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> SshStore {
        SshStore {
            vault: CryptoVault::for_test(),
            conns: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    #[test]
    fn test_encrypt_secret_roundtrip() {
        let store = test_store();
        let input = SshConnectionInput {
            id: None,
            name: "prod".into(),
            host: "10.0.0.1".into(),
            port: 22,
            username: "root".into(),
            auth_type: "password".into(),
            secret: "s3cret".into(),
            key_passphrase: None,
        };
        let (enc, pass) = store.encrypt_secret(&input).unwrap();
        assert_ne!(enc, "s3cret");
        assert!(pass.is_none());
        assert_eq!(store.vault.decrypt(&enc).unwrap(), "s3cret");
    }

    #[test]
    fn test_info_excludes_secret() {
        let conn = SshConnection {
            id: "1".into(),
            name: "prod".into(),
            host: "h".into(),
            port: 22,
            username: "u".into(),
            auth_type: "password".into(),
            encrypted_secret: "TOP-SECRET".into(),
            key_passphrase_encrypted: None,
            created_at: 0,
            updated_at: 0,
        };
        let info: SshConnectionInfo = conn.into();
        assert_eq!(info.id, "1");
        assert!(!serde_json::to_string(&info).unwrap().contains("TOP-SECRET"));
    }
}
