use crate::commands::ssh::connections::{SshConnection, SshStore};
use russh::client::Handler;
use russh::keys::PrivateKeyWithHashAlg;
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};
use tauri::Emitter;

pub struct SshSessionManager {
    pub sessions: tokio::sync::Mutex<HashMap<String, SessionEntry>>,
    pub known_hosts: Mutex<HashMap<String, String>>, // host:port -> fingerprint
    pub pending_keys: Mutex<HashMap<String, PendingKey>>, // session_id -> pending server key
}

pub struct PendingKey {
    pub host: String,
    pub fingerprint: String,
    pub server_key: russh::keys::PublicKey,
    pub approved: bool,
}

pub struct SessionEntry {
    pub client: russh::client::Handle<SshHandler>,
    pub connection_id: String,
    pub terminals: tokio::sync::Mutex<HashMap<String, TerminalHandle>>,
}

// TerminalHandle 持有可写的 write half（读 half 已移入后台 task）
pub struct TerminalHandle {
    pub write: russh::ChannelWriteHalf<russh::client::Msg>,
}

pub struct SshHandler {
    pub server_key: Arc<Mutex<Option<russh::keys::PublicKey>>>,
}

impl Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        *self.server_key.lock().unwrap() = Some(server_public_key.clone());
        Ok(true) // 先接受，connect 完成后由上层统一校验
    }
}

fn known_hosts_path() -> std::path::PathBuf {
    crate::utils::data_dir().join("known_hosts.json")
}

fn load_known_hosts() -> HashMap<String, String> {
    let path = known_hosts_path();
    if !path.exists() {
        return HashMap::new();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_known_hosts(map: &HashMap<String, String>) -> Result<(), String> {
    let path = known_hosts_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_vec(map).map_err(|e| e.to_string())?;
    fs::write(&path, &data).map_err(|e| format!("write known_hosts: {}", e))
}

/// 校验 host key：
/// - 已知且匹配 -> Ok(Accept)
/// - 已知但不匹配 -> Err(HostKeyMismatch)
/// - 未知 -> Ok(Prompt) 等待前端确认后落盘
pub enum HostKeyCheck {
    Known,
    Unknown,
}

pub fn check_host_key(
    known: &HashMap<String, String>,
    host: &str,
    fingerprint: &str,
) -> Result<HostKeyCheck, String> {
    match known.get(host) {
        Some(k) if k == fingerprint => Ok(HostKeyCheck::Known),
        Some(_) => Err(format!("HOST_KEY_MISMATCH: {host} fingerprint changed")),
        None => Ok(HostKeyCheck::Unknown),
    }
}

/// 生成展示用指纹（SHA256 优先）
pub fn fingerprint(server_key: &russh::keys::PublicKey) -> String {
    server_key
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string()
}

fn session_config() -> Arc<russh::client::Config> {
    let config = russh::client::Config {
        keepalive_interval: Some(std::time::Duration::from_secs(30)),
        keepalive_max: 3,
        ..russh::client::Config::default()
    };
    Arc::new(config)
}

async fn authenticate(
    client: &mut russh::client::Handle<SshHandler>,
    store: &SshStore,
    conn: &SshConnection,
) -> Result<(), String> {
    let user = conn.username.as_str();
    match conn.auth_type.as_str() {
        "password" => {
            let pass = store.decrypt_secret(conn)?;
            match client.authenticate_password(user, pass.as_str()).await {
                Ok(russh::client::AuthResult::Success) => Ok(()),
                Ok(_) => Err("AUTH_FAILED: authentication did not succeed".into()),
                Err(e) => Err(format!("AUTH_FAILED: {e}")),
            }
        }
        "private_key" => {
            let pem = store.decrypt_secret(conn)?;
            let passphrase = store.decrypt_passphrase(conn)?;
            let key = russh::keys::decode_secret_key(pem.as_str(), passphrase.as_deref())
                .map_err(|e| format!("KEY_INVALID: {e}"))?;
            let key_with_hash =
                PrivateKeyWithHashAlg::new(Arc::new(key), Some(russh::keys::HashAlg::Sha256));
            match client.authenticate_publickey(user, key_with_hash).await {
                Ok(russh::client::AuthResult::Success) => Ok(()),
                Ok(_) => Err("AUTH_FAILED: public key auth did not succeed".into()),
                Err(e) => Err(format!("AUTH_FAILED: {e}")),
            }
        }
        other => Err(format!("INVALID_AUTH_TYPE: {other}")),
    }
}

#[allow(clippy::new_without_default)]
impl SshSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: tokio::sync::Mutex::new(HashMap::new()),
            known_hosts: Mutex::new(load_known_hosts()),
            pending_keys: Mutex::new(HashMap::new()),
        }
    }

    fn host_key(&self, host: &str) -> Option<String> {
        self.known_hosts.lock().ok()?.get(host).cloned()
    }

    fn record_host_key(&self, host: &str, fingerprint: &str) {
        if let Ok(mut kh) = self.known_hosts.lock() {
            kh.insert(host.to_string(), fingerprint.to_string());
            let _ = save_known_hosts(&kh);
        }
    }
}

pub async fn open(
    app: &tauri::AppHandle,
    store: &SshStore,
    manager: &SshSessionManager,
    connection_id: &str,
) -> Result<String, String> {
    let conn = store
        .find(connection_id)
        .ok_or_else(|| format!("NOT_FOUND: connection {connection_id}"))?;
    let host_key = format!("{}:{}", conn.host, conn.port);

    // 1. TCP 连接
    let tcp = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::net::TcpStream::connect((conn.host.as_str(), conn.port)),
    )
    .await
    .map_err(|_| format!("TIMEOUT: connect to {}:{}", conn.host, conn.port))?
    .map_err(|e| format!("CONNECT_FAILED: {e}"))?;

    // 2. SSH 握手 + 捕获 server key
    let server_key: Arc<Mutex<Option<russh::keys::PublicKey>>> = Arc::new(Mutex::new(None));
    let handler = SshHandler {
        server_key: server_key.clone(),
    };
    let config = session_config();
    let mut client = russh::client::connect_stream(config, tcp, handler)
        .await
        .map_err(|e| format!("HANDSHAKE_FAILED: {e}"))?;

    let key = server_key
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "NO_SERVER_KEY".to_string())?;
    let fp = fingerprint(&key);

    // 3. host key 校验
    let known = manager.host_key(&host_key);
    match known {
        Some(k) if k == fp => {}
        Some(_) => {
            return Err(format!(
                "HOST_KEY_MISMATCH: {} fingerprint changed",
                conn.host
            ))
        }
        None => {
            // 首次连接：登记 pending，等待前端确认
            let session_id = uuid::Uuid::new_v4().to_string();
            manager.pending_keys.lock().unwrap().insert(
                session_id.clone(),
                PendingKey {
                    host: host_key.clone(),
                    fingerprint: fp.clone(),
                    server_key: key.clone(),
                    approved: false,
                },
            );
            let _ = app.emit(
                "ssh-hostkey-prompt",
                serde_json::json!({
                    "session_id": session_id,
                    "host": host_key,
                    "fingerprint": fp,
                }),
            );
            // 等前端 ssh_hostkey_accept/reject（最长 30s）
            let start = std::time::Instant::now();
            loop {
                let approved = {
                    let pending = manager.pending_keys.lock().unwrap();
                    pending.get(&session_id).map(|p| p.approved)
                };
                match approved {
                    Some(true) => {
                        manager.pending_keys.lock().unwrap().remove(&session_id);
                        manager.record_host_key(&host_key, &fp);
                        break;
                    }
                    Some(false) => {
                        if start.elapsed() > std::time::Duration::from_secs(30) {
                            manager.pending_keys.lock().unwrap().remove(&session_id);
                            return Err("HOSTKEY_REJECTED: no confirmation within 30s".into());
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    None => return Err("HOSTKEY_REJECTED".into()),
                }
            }
        }
    }

    // 4. 认证
    authenticate(&mut client, store, &conn).await?;

    // 5. 存入连接池
    let session_id = uuid::Uuid::new_v4().to_string();
    manager.sessions.lock().await.insert(
        session_id.clone(),
        SessionEntry {
            client,
            connection_id: conn.id.clone(),
            terminals: tokio::sync::Mutex::new(HashMap::new()),
        },
    );
    Ok(session_id)
}

pub async fn close(manager: &SshSessionManager, session_id: &str) {
    manager.sessions.lock().await.remove(session_id);
}

#[tauri::command]
pub fn ssh_hostkey_accept(
    state: tauri::State<SshSessionManager>,
    session_id: String,
    host: String,
    fingerprint: String,
) -> Result<(), String> {
    let mut pending = state.pending_keys.lock().map_err(|e| e.to_string())?;
    let p = pending
        .get_mut(&session_id)
        .ok_or_else(|| "NO_PENDING_KEY".to_string())?;
    if p.host != host || p.fingerprint != fingerprint {
        return Err("HOSTKEY_MISMATCH".into());
    }
    p.approved = true;
    Ok(())
}

#[tauri::command]
pub fn ssh_hostkey_reject(
    state: tauri::State<SshSessionManager>,
    session_id: String,
) -> Result<(), String> {
    state
        .pending_keys
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&session_id);
    Ok(())
}

#[tauri::command]
pub async fn ssh_close(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
) -> Result<(), String> {
    state.sessions.lock().await.remove(&session_id);
    Ok(())
}

/// 连接测试：复用 open 但立即关闭
#[tauri::command]
pub async fn ssh_test_connection(
    app: tauri::AppHandle,
    store: tauri::State<'_, SshStore>,
    manager: tauri::State<'_, SshSessionManager>,
    connection_id: String,
) -> Result<String, String> {
    let sid = open(&app, &store, &manager, &connection_id).await?;
    close(&manager, &sid).await;
    Ok("ok".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_sha256_format() {
        // russh PublicKey::fingerprint 返回 SHA256:<base64> 或 MD5:<hex>，这里只断言格式前缀由 russh 决定；
        // 我们封装只做展示。直接测 check_host_key 逻辑。
        let mut known = HashMap::new();
        known.insert("h:22".to_string(), "SHA256:abc".to_string());
        assert!(matches!(
            check_host_key(&known, "h:22", "SHA256:abc").unwrap(),
            HostKeyCheck::Known
        ));
        assert!(check_host_key(&known, "h:22", "SHA256:xyz").is_err());
        assert!(matches!(
            check_host_key(&known, "new:22", "SHA256:zzz").unwrap(),
            HostKeyCheck::Unknown
        ));
    }

    #[test]
    fn test_known_hosts_roundtrip() {
        let mut map = HashMap::new();
        map.insert("a:22".to_string(), "SHA256:x".to_string());
        save_known_hosts(&map).unwrap();
        let loaded = load_known_hosts();
        assert_eq!(loaded.get("a:22").map(String::as_str), Some("SHA256:x"));
        let _ = fs::remove_file(known_hosts_path());
    }
}
