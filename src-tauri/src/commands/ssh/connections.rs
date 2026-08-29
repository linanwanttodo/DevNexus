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
    /// 连接分组（如 "生产环境"、"测试环境"）
    pub group: Option<String>,
    /// 标签列表
    pub tags: Vec<String>,
    /// 上次成功连接的时间戳
    pub last_connected: Option<i64>,
    /// Keepalive 间隔（秒），默认 30
    pub keepalive_secs: u64,
    /// 跳板机 ID（指向另一个 SshConnection.id）
    pub jump_host_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SshConnectionInfo {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub group: Option<String>,
    pub tags: Vec<String>,
    pub last_connected: Option<i64>,
    pub keepalive_secs: u64,
    pub jump_host_id: Option<String>,
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
    pub group: Option<String>,
    pub tags: Vec<String>,
    pub keepalive_secs: Option<u64>,
    pub jump_host_id: Option<String>,
}

/// OpenSSH config 解析后的单条记录
#[derive(Debug, Clone, Serialize)]
pub struct OpenSshHost {
    pub host: String,
    pub host_name: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub proxy_command: Option<String>,
    pub proxy_jump: Option<String>,
}

pub struct SshStore {
    pub vault: CryptoVault,
    pub conns: Arc<Mutex<Vec<SshConnection>>>,
    pub next_id: Arc<Mutex<u64>>,
    /// 是否已从磁盘加载过（惰性加载用）
    loaded: std::sync::atomic::AtomicBool,
}

#[allow(clippy::new_without_default)]
impl SshStore {
    pub fn new() -> Self {
        // 惰性加载：构造时不再 read 磁盘，首次访问命令时通过 ensure_loaded() 加载，
        // 未使用 SSH 功能的应用启动不再读入 ssh_connections.json，省去磁盘/内存开销。
        Self {
            vault: CryptoVault::new(),
            conns: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
            loaded: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// 确保连接列表已加载（惰性初始化入口）。所有需要读取/修改 conns 的命令应先行调用。
    /// 用 CAS 保证并发下只加载一次。
    pub fn ensure_loaded(&self) -> Result<(), String> {
        use std::sync::atomic::Ordering;
        if self.loaded.load(Ordering::Acquire) {
            return Ok(());
        }
        self.load()?;
        self.loaded.store(true, Ordering::Release);
        Ok(())
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
            group: c.group.clone(),
            tags: c.tags.clone(),
            last_connected: c.last_connected,
            keepalive_secs: c.keepalive_secs,
            jump_host_id: c.jump_host_id.clone(),
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
    state.ensure_loaded()?;
    let conns = state.conns.lock().map_err(|e| e.to_string())?;
    Ok(conns.iter().map(SshStore::to_info).collect())
}

#[tauri::command]
pub fn ssh_save_connection(
    state: tauri::State<SshStore>,
    connection: SshConnectionInput,
) -> Result<SshConnectionInfo, String> {
    state.ensure_loaded()?;
    let (enc, pass) = state.encrypt_secret(&connection)?;
    let ts = now_ts();
    let mut conns = state.conns.lock().map_err(|e| e.to_string())?;
    let conn = match &connection.id {
        Some(id) if conns.iter().any(|c| &c.id == id) => {
            let c = conns.iter_mut().find(|c| &c.id == id).unwrap();
            c.name = connection.name.clone();
            c.host = connection.host.clone();
            c.port = connection.port;
            c.username = connection.username.clone();
            c.auth_type = connection.auth_type.clone();
            c.encrypted_secret = enc;
            c.key_passphrase_encrypted = pass;
            c.updated_at = ts;
            c.group = connection.group.clone();
            c.tags = connection.tags.clone();
            c.keepalive_secs = connection.keepalive_secs.unwrap_or(30);
            c.jump_host_id = connection.jump_host_id.clone();
            c.clone()
        }
        _ => {
            let mut nid = state.next_id.lock().map_err(|e| e.to_string())?;
            let id = nid.to_string();
            *nid += 1;
            let c = SshConnection {
                id: id.clone(),
                name: connection.name.clone(),
                host: connection.host.clone(),
                port: connection.port,
                username: connection.username.clone(),
                auth_type: connection.auth_type.clone(),
                encrypted_secret: enc,
                key_passphrase_encrypted: pass,
                created_at: ts,
                updated_at: ts,
                group: connection.group.clone(),
                tags: connection.tags.clone(),
                last_connected: None,
                keepalive_secs: connection.keepalive_secs.unwrap_or(30),
                jump_host_id: connection.jump_host_id.clone(),
            };
            conns.push(c.clone());
            c
        }
    };
    drop(conns);
    state.save()?;
    Ok(SshStore::to_info(&conn))
}

/// 标记连接成功建立的时间戳（session 打开成功后调用）
#[tauri::command]
pub fn ssh_touch_connection(state: tauri::State<SshStore>, id: String) -> Result<(), String> {
    state.ensure_loaded()?;
    let ts = now_ts();
    let mut conns = state.conns.lock().map_err(|e| e.to_string())?;
    if let Some(c) = conns.iter_mut().find(|c| c.id == id) {
        c.last_connected = Some(ts);
    }
    drop(conns);
    state.save()
}

/// 从 ~/.ssh/config 解析连接条目（不包含密钥，仅元数据）
#[tauri::command]
pub fn ssh_import_open_ssh_config() -> Result<Vec<OpenSshHost>, String> {
    let home = dirs::home_dir().ok_or("cannot find home directory")?;
    let config_path = home.join(".ssh").join("config");
    if !config_path.exists() {
        return Ok(Vec::new());
    }
    let content =
        std::fs::read_to_string(&config_path).map_err(|e| format!("read config: {}", e))?;
    parse_openssh_config(&content)
}

/// 解析 OpenSSH config 内容，返回 Host 条目列表
fn parse_openssh_config(content: &str) -> Result<Vec<OpenSshHost>, String> {
    let mut hosts = Vec::new();
    // 当前 Host 块内的全部别名：一条 `Host a b` 会生成多个别名且属性共享
    let mut current: Vec<OpenSshHost> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let key = parts[0].to_lowercase();
        let value = parts[1..].join(" ");

        match key.as_str() {
            "host" => {
                hosts.append(&mut current);
                // 通配（含 * ?）与取反（! 开头）的别名不作为具体连接导入，
                // 否则 `Host *` 等 catch-all 会污染导入列表
                for token in parts[1..].iter() {
                    if token.contains('*') || token.contains('?') || token.starts_with('!') {
                        continue;
                    }
                    current.push(OpenSshHost {
                        host: token.to_string(),
                        host_name: None,
                        user: None,
                        port: None,
                        identity_file: None,
                        proxy_command: None,
                        proxy_jump: None,
                    });
                }
            }
            "match" => {
                // Match 条件块：固化已收集的主机并腾空 current，
                // 使块内属性无处附着而被跳过（append 后 current 已为空）
                hosts.append(&mut current);
            }
            _ => {
                for h in current.iter_mut() {
                    apply_openssh_attr(h, key.as_str(), &value);
                }
            }
        }
    }
    hosts.append(&mut current);
    Ok(hosts)
}

/// 把 OpenSSH config 的单个属性应用到一条 Host 记录
fn apply_openssh_attr(h: &mut OpenSshHost, key: &str, value: &str) {
    match key {
        "hostname" => h.host_name = Some(value.to_string()),
        "user" => h.user = Some(value.to_string()),
        "port" => h.port = value.parse().ok(),
        "identityfile" | "identity_file" => {
            // 展开 ~ 为真实 home 路径
            let path = if value.starts_with("~/") {
                dirs::home_dir()
                    .map(|home| home.join(value.trim_start_matches("~/")))
                    .unwrap_or_else(|| std::path::PathBuf::from(value))
            } else {
                std::path::PathBuf::from(value)
            };
            h.identity_file = Some(path.to_string_lossy().into_owned());
        }
        "proxycommand" => h.proxy_command = Some(value.to_string()),
        "proxyjump" => h.proxy_jump = Some(value.to_string()),
        _ => {}
    }
}

/// 导出连接为 OpenSSH config 格式（不含密码，仅元数据）
#[tauri::command]
pub fn ssh_export_openssh_config(
    conn_ids: Vec<String>,
    state: tauri::State<SshStore>,
) -> Result<String, String> {
    state.ensure_loaded()?;
    let conns = state.conns.lock().map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    for c in conns.iter().filter(|c| conn_ids.contains(&c.id)) {
        lines.push(format!("Host {}", c.name));
        if let Some(ref g) = c.group {
            lines.push(format!("  # group: {}", g));
        }
        lines.push(format!("  HostName {}", c.host));
        lines.push(format!("  User {}", c.username));
        if c.port != 22 {
            lines.push(format!("  Port {}", c.port));
        }
        lines.push(String::new());
    }
    Ok(lines.join("\n"))
}

#[tauri::command]
pub fn ssh_delete_connection(state: tauri::State<SshStore>, id: String) -> Result<(), String> {
    state.ensure_loaded()?;
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
            loaded: std::sync::atomic::AtomicBool::new(false),
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
            group: None,
            tags: Vec::new(),
            keepalive_secs: None,
            jump_host_id: None,
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
            group: None,
            tags: Vec::new(),
            last_connected: None,
            keepalive_secs: 30,
            jump_host_id: None,
        };
        let info: SshConnectionInfo = conn.into();
        assert_eq!(info.id, "1");
        assert!(!serde_json::to_string(&info).unwrap().contains("TOP-SECRET"));
    }

    #[test]
    fn test_parse_openssh_skips_wildcard_and_match() {
        let cfg = "Host *\n  User root\n\nHost web\n  HostName 10.0.0.1\n  User deploy\n\nMatch all\n  User ignored\n  HostName 1.2.3.4\n";
        let hosts = parse_openssh_config(cfg).unwrap();
        // `Host *` 与 `Match` 块都不应产生连接；仅 `web` 被导入
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].host, "web");
        assert_eq!(hosts[0].host_name.as_deref(), Some("10.0.0.1"));
        assert_eq!(hosts[0].user.as_deref(), Some("deploy"));
    }

    #[test]
    fn test_parse_openssh_multi_alias_shares_attrs() {
        let cfg = "Host web1 web2\n  HostName 10.0.0.1\n  Port 2222\n";
        let hosts = parse_openssh_config(cfg).unwrap();
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].host, "web1");
        assert_eq!(hosts[1].host, "web2");
        for h in &hosts {
            assert_eq!(h.host_name.as_deref(), Some("10.0.0.1"));
            assert_eq!(h.port, Some(2222));
        }
    }

    #[test]
    fn test_parse_openssh_negated_alias_skipped() {
        let cfg = "Host !secret public\n  HostName 10.0.0.9\n";
        let hosts = parse_openssh_config(cfg).unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].host, "public");
    }
}
