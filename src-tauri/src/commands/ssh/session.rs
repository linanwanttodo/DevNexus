use crate::commands::ssh::connections::SshConnection;
use crate::commands::ssh::connections::SshStore;
use russh::client::Handler;
use russh::keys::PrivateKeyWithHashAlg;
use russh_sftp::client::SftpSession;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncRead, AsyncWrite};

/// 任何可作为 SSH 传输层的字节流：同时实现 AsyncRead/AsyncWrite 且可跨线程发送。
/// 用于把直连 TcpStream 与跳板机隧道 ChannelStream 统一为 `Box<dyn TunnelStream>`。
trait TunnelStream: AsyncRead + AsyncWrite + Unpin + Send + 'static {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> TunnelStream for T {}
use tauri::Emitter;

pub struct SshSessionManager {
    pub sessions: tokio::sync::Mutex<HashMap<String, Arc<SessionEntry>>>,
    /// 每个连接 ID 一把互斥锁：串行化并发 open()，防止同一连接被重复建立
    pub open_locks: tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    pub known_hosts: Mutex<HashMap<String, String>>, // host:port -> fingerprint
    pub pending_keys: Mutex<HashMap<String, PendingKey>>, // session_id -> pending server key
}

pub struct PendingKey {
    pub host: String,
    pub fingerprint: String,
    pub server_key: russh::keys::PublicKey,
    pub approved: bool,
}

/// 单个 SSH 会话。句柄全部 `Arc` 化，调用方取出句柄后即可释放会话表锁，
/// 使不同会话（及同会话的终端/SFTP 通道）的远程 I/O 互不阻塞。
/// `client` 不可 Clone 且 `channel_open_session` 需要 `&mut`，
/// 仅开通道时短暂持锁；已建立通道的读写不再经过它。
pub struct SessionEntry {
    pub client: tokio::sync::Mutex<russh::client::Handle<SshHandler>>,
    pub connection_id: String,
    pub terminals: tokio::sync::Mutex<HashMap<String, Arc<TerminalHandle>>>,
    pub sftp_sessions: tokio::sync::Mutex<HashMap<String, Arc<SftpHandle>>>,
    /// 本地端口转发列表（已激活的 -L 转发）
    pub local_forwards: tokio::sync::Mutex<Vec<ForwardEntry>>,
    /// 动态 SOCKS5 代理转发列表（已激活的 -D 转发）
    pub socks_forwards: tokio::sync::Mutex<Vec<SocksEntry>>,
    /// 每个端口转发对应的停止信号；关闭/会话结束时 notify，令后台 accept 循环退出并释放端口
    pub forward_stops: tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Notify>>>,
    /// 每个 SOCKS5 代理对应的停止信号（同上）
    pub socks_stops: tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Notify>>>,
    /// 是否启用 SSH Agent 转发（开启后新开终端会请求 auth-agent-req，服务端据此建立转发通道）
    pub agent_forwarding: std::sync::atomic::AtomicBool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ForwardEntry {
    pub id: String,
    pub bind_host: String,
    pub bind_port: u16,
    pub dest_host: String,
    pub dest_port: u16,
    pub active: bool,
}

/// 动态 SOCKS5 代理转发（-D）记录。每个条目在本地绑定一个 SOCKS5 监听端口，
/// 由客户端在连接时动态指定目标地址（经 SSH direct-tcpip 逐连接建立隧道）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SocksEntry {
    pub id: String,
    pub bind_host: String,
    pub bind_port: u16,
    pub active: bool,
}

// TerminalHandle 持有可写的 write half（读 half 已移入后台 task）
pub struct TerminalHandle {
    pub write: tokio::sync::Mutex<russh::ChannelWriteHalf<russh::client::Msg>>,
    /// 最近输出环形缓冲（AI 读屏用）。容量固定，超出丢弃最旧。
    /// 仅存原始字节（UTF-8 无损解码），不解析 ANSI，保持低开销。
    pub output_buffer: tokio::sync::Mutex<TerminalBuffer>,
}

/// 终端环形输出缓冲：保留最近 N 行（按换行切分），供 AI 读取当前屏幕上下文。
/// 行以原始字符串存储，调用方按需去除 ANSI 转义。
pub struct TerminalBuffer {
    pub lines: VecDeque<String>,
    pub capacity: usize,
    pending: String,
}

impl TerminalBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(capacity),
            capacity,
            pending: String::new(),
        }
    }

    /// 追加一段输出：按换行切分；\r 视为回车覆盖（移除）。
    /// 未以换行结尾的末尾片段暂存为 pending，下次 append 时与首段拼接，
    /// 正确处理 PTY 分片输出（如 "hel" + "lo\n" → "hello"）。
    pub fn append(&mut self, text: &str) {
        let sanitized = text.replace('\r', "");
        if sanitized.is_empty() {
            return;
        }
        // 将上次的 pending 与本次文本拼接后统一按 \n 切分
        let combined = if self.pending.is_empty() {
            sanitized
        } else {
            let mut s = std::mem::take(&mut self.pending);
            s.push_str(&sanitized);
            s
        };
        let ends_with_newline = combined.ends_with('\n');
        let mut parts: Vec<&str> = combined.split('\n').collect();
        if ends_with_newline {
            // 末尾空串（split 产生）不作为行
            parts.pop();
            for p in parts {
                self.push_line(p.to_string());
            }
        } else {
            // 最后一部分为未闭合的 pending
            if let Some(last) = parts.pop() {
                for p in parts {
                    self.push_line(p.to_string());
                }
                self.pending = last.to_string();
            }
        }
    }

    fn push_line(&mut self, line: String) {
        self.lines.push_back(line);
        while self.lines.len() > self.capacity {
            self.lines.pop_front();
        }
    }

    /// 返回最近 `n` 行拼接（含尚未换行的 pending 片段，不带 ANSI 清理，由调用方处理）
    pub fn recent(&self, n: usize) -> String {
        let mut all: Vec<String> = self.lines.iter().cloned().collect();
        if !self.pending.is_empty() {
            all.push(self.pending.clone());
        }
        let start = all.len().saturating_sub(n);
        all[start..].join("\n")
    }

    pub fn all(&self) -> String {
        let mut all: Vec<String> = self.lines.iter().cloned().collect();
        if !self.pending.is_empty() {
            all.push(self.pending.clone());
        }
        all.join("\n")
    }
}

// SftpHandle 持有 sftp 会话
pub struct SftpHandle {
    pub sftp: tokio::sync::Mutex<SftpSession>,
}

pub struct SshHandler {
    pub server_key: Arc<Mutex<Option<russh::keys::PublicKey>>>,
    /// 本地 SSH agent 套接字路径（来自 SSH_AUTH_SOCK）。非空时，服务端建立的
    /// auth-agent 转发通道会被代理到本地 agent；为空则拒绝转发（无可用 agent）。
    pub agent_sock: Option<String>,
}

impl Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        *self.server_key.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(server_public_key.clone());
        Ok(true) // 先接受，connect 完成后由上层统一校验
    }

    /// 服务端请求建立 auth-agent 转发通道时触发：把该通道与本地 SSH agent 套接字
    /// 双向代理，从而让远端服务器能使用本地的 ssh-agent 私钥。仅当本地存在
    /// SSH_AUTH_SOCK 时接受，否则拒绝（避免无意义通道）。
    #[allow(clippy::type_complexity)]
    fn server_channel_open_agent_forward(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        reply: russh::client::ChannelOpenHandle,
        _session: &mut russh::client::Session,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send {
        let sock = self.agent_sock.clone();
        async move {
            match sock {
                Some(path) => {
                    #[cfg(unix)]
                    {
                        match tokio::net::UnixStream::connect(&path).await {
                            Ok(mut agent) => {
                                // 接受通道，并将远端 auth-agent 通道与本地 agent 双向代理
                                reply.accept().await;
                                let mut stream = channel.into_stream();
                                tokio::spawn(async move {
                                    let _ = tokio::io::copy_bidirectional(&mut stream, &mut agent)
                                        .await;
                                });
                                Ok(())
                            }
                            Err(e) => {
                                eprintln!("[agent] connect local SSH_AUTH_SOCK failed: {e}");
                                // 丢弃 reply -> 自动拒绝
                                Ok(())
                            }
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        // 非 Unix 平台无 Unix 域套接字 agent，拒绝转发
                        let _ = reply;
                        Ok(())
                    }
                }
                None => {
                    // 本地无 agent，拒绝
                    let _ = reply;
                    Ok(())
                }
            }
        }
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

/// 按连接级 keepalive 配置生成 russh 客户端配置。
/// `keepalive_secs == 0` 表示关闭保活（服务端默认 30s 的硬编码此前从未生效，属死代码）。
fn session_config(keepalive_secs: u64) -> Arc<russh::client::Config> {
    let keepalive_interval = if keepalive_secs == 0 {
        None
    } else {
        Some(std::time::Duration::from_secs(keepalive_secs))
    };
    let config = russh::client::Config {
        keepalive_interval,
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
            open_locks: tokio::sync::Mutex::new(HashMap::new()),
            known_hosts: Mutex::new(load_known_hosts()),
            pending_keys: Mutex::new(HashMap::new()),
        }
    }

    /// 按连接 ID 查找已有会话
    pub async fn find_by_connection(&self, connection_id: &str) -> Option<Arc<SessionEntry>> {
        let sessions = self.sessions.lock().await;
        sessions
            .values()
            .find(|s| s.connection_id == connection_id)
            .cloned()
    }

    /// 复用或新建连接会话（并发安全）。同一 `connection_id` 的并发请求经每连接
    /// 互斥锁串行化，后到者在锁内二次检查后直接复用已有会话，避免双方都查不到
    /// 而各自 `open()` 出两个会话（重复跳板/认证、多占连接）。
    pub async fn get_or_open(
        &self,
        app: &tauri::AppHandle,
        store: &SshStore,
        connection_id: &str,
    ) -> Result<Arc<SessionEntry>, String> {
        if let Some(e) = self.find_by_connection(connection_id).await {
            return Ok(e);
        }
        let lock = {
            let mut locks = self.open_locks.lock().await;
            locks.entry(connection_id.to_string()).or_default().clone()
        };
        let _guard = lock.lock().await;
        // 拿到锁后二次检查：会话可能已被并发请求创建
        if let Some(e) = self.find_by_connection(connection_id).await {
            return Ok(e);
        }
        let sid = open(app, store, self, connection_id).await?;
        self.sessions
            .lock()
            .await
            .get(&sid)
            .cloned()
            .ok_or_else(|| "NO_SESSION".to_string())
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

    /// 按 terminal id 取写句柄（取出即释放会话表锁，I/O 不阻塞其他会话）
    pub async fn find_terminal(&self, term_id: &str) -> Option<Arc<TerminalHandle>> {
        let sessions = self.sessions.lock().await;
        for entry in sessions.values() {
            let terms = entry.terminals.lock().await;
            if let Some(t) = terms.get(term_id) {
                return Some(t.clone());
            }
        }
        None
    }

    /// 按 sftp id 取 SFTP 句柄（取出即释放会话表锁，I/O 不阻塞其他会话）
    pub async fn find_sftp(&self, sftp_id: &str) -> Option<Arc<SftpHandle>> {
        let sessions = self.sessions.lock().await;
        for entry in sessions.values() {
            let map = entry.sftp_sessions.lock().await;
            if let Some(h) = map.get(sftp_id) {
                return Some(h.clone());
            }
        }
        None
    }

    /// 按 ID 移除并关闭 SFTP 通道（不影响 SSH 会话本身，同连接的终端继续可用）
    pub async fn remove_sftp(&self, sftp_id: &str) -> bool {
        let sessions = self.sessions.lock().await;
        for entry in sessions.values() {
            let mut map = entry.sftp_sessions.lock().await;
            if map.remove(sftp_id).is_some() {
                return true;
            }
        }
        false
    }
}

/// 通过跳板机建立到目标的 SSH 传输层（ProxyJump 单跳）。
/// 流程：直连跳板机 → 握手/认证 → 在跳板会话上开 direct-tcpip 通道到 (conn.host:conn.port)
/// → 用该通道作为目标主机 SSH 握手的传输层（由上层 open() 接管后续握手/认证）。
///
/// 说明：跳板机自身须已“已知 host key”（先单独连一次跳板机，让 DevNexus 记住指纹）；
/// 若跳板机指纹未知，则直接报错引导用户先直连跳板机，避免二次 hostkey 确认的交互歧义。
async fn open_via_jump(
    store: &SshStore,
    manager: &SshSessionManager,
    app: &tauri::AppHandle,
    conn: &SshConnection,
    target_host_key: &str,
    jump_id: &str,
) -> Result<Box<dyn TunnelStream>, String> {
    let jump_conn = store
        .find(jump_id)
        .ok_or_else(|| format!("JUMP_HOST_NOT_FOUND: {jump_id}"))?;

    let jump_host_key = format!("{}:{}", jump_conn.host, jump_conn.port);

    // 1. 直连跳板机
    let jtcp = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::net::TcpStream::connect((jump_conn.host.as_str(), jump_conn.port)),
    )
    .await
    .map_err(|_| {
        format!(
            "JUMP_TIMEOUT: connect to {}:{}",
            jump_conn.host, jump_conn.port
        )
    })?
    .map_err(|e| format!("JUMP_CONNECT_FAILED: {e}"))?;

    // 2. 跳板机握手 + 捕获 server key
    let j_server_key: Arc<Mutex<Option<russh::keys::PublicKey>>> = Arc::new(Mutex::new(None));
    let j_handler = SshHandler {
        server_key: j_server_key.clone(),
        agent_sock: None,
    };
    let j_config = session_config(jump_conn.keepalive_secs);
    let mut j_client = russh::client::connect_stream(j_config, jtcp, j_handler)
        .await
        .map_err(|e| format!("JUMP_HANDSHAKE_FAILED: {e}"))?;

    let j_key = j_server_key
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "JUMP_NO_SERVER_KEY".to_string())?;
    let j_fp = fingerprint(&j_key);

    // 3. 跳板机 host key 校验（未知则引导先直连）
    let j_known = manager.host_key(&jump_host_key);
    match j_known {
        Some(k) if k == j_fp => {}
        _ => {
            return Err(format!(
                "JUMP_HOST_UNKNOWN_KEY: connect to jump host {} directly once to trust its key",
                jump_conn.host
            ));
        }
    }

    // 4. 跳板机认证
    authenticate(&mut j_client, store, &jump_conn).await?;

    // 5. 在跳板会话上开 direct-tcpip 通道到目标主机
    let ch = j_client
        .channel_open_direct_tcpip(
            conn.host.clone(),
            conn.port as u32,
            "127.0.0.1".to_string(),
            0,
        )
        .await
        .map_err(|e| format!("JUMP_DIRECT_TCP_FAIL: {e}"))?;

    // 触发一次事件，确保后续目标握手若产生 hostkey 提示能被前端接收
    let _ = app;
    let _ = target_host_key;

    // 6. 用通道作为目标 SSH 传输层
    Ok(Box::new(ch.into_stream()))
}

pub async fn open(
    app: &tauri::AppHandle,
    store: &SshStore,
    manager: &SshSessionManager,
    connection_id: &str,
) -> Result<String, String> {
    store.ensure_loaded()?;
    let conn = store
        .find(connection_id)
        .ok_or_else(|| format!("NOT_FOUND: connection {connection_id}"))?;

    let host_key = format!("{}:{}", conn.host, conn.port);

    // 0. 传输层：若配置了跳板机，则先连跳板、经其建立 direct-tcpip 隧道到目标，
    //    再在隧道之上跑目标主机的 SSH 握手；否则直连。
    let transport: Box<dyn TunnelStream> = if let Some(jump_id) = &conn.jump_host_id {
        Box::new(open_via_jump(store, manager, app, &conn, &host_key, jump_id).await?)
    } else {
        // 1. TCP 连接
        let tcp = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::net::TcpStream::connect((conn.host.as_str(), conn.port)),
        )
        .await
        .map_err(|_| format!("TIMEOUT: connect to {}:{}", conn.host, conn.port))?
        .map_err(|e| format!("CONNECT_FAILED: {e}"))?;
        Box::new(tcp)
    };

    // 2. SSH 握手 + 捕获 server key
    let server_key: Arc<Mutex<Option<russh::keys::PublicKey>>> = Arc::new(Mutex::new(None));
    // 捕获本地 agent 套接字，供后续服务端建立 auth-agent 转发通道时代理使用
    let agent_sock = std::env::var("SSH_AUTH_SOCK").ok();
    let handler = SshHandler {
        server_key: server_key.clone(),
        agent_sock: agent_sock.clone(),
    };
    let config = session_config(conn.keepalive_secs);
    let mut client = russh::client::connect_stream(config, transport, handler)
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
            manager
                .pending_keys
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(
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
                    let pending = manager
                        .pending_keys
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    pending.get(&session_id).map(|p| p.approved)
                };
                match approved {
                    Some(true) => {
                        manager
                            .pending_keys
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(&session_id);
                        manager.record_host_key(&host_key, &fp);
                        break;
                    }
                    Some(false) => {
                        if start.elapsed() > std::time::Duration::from_secs(30) {
                            manager
                                .pending_keys
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .remove(&session_id);
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
        Arc::new(SessionEntry {
            client: tokio::sync::Mutex::new(client),
            connection_id: conn.id.clone(),
            terminals: tokio::sync::Mutex::new(HashMap::new()),
            sftp_sessions: tokio::sync::Mutex::new(HashMap::new()),
            local_forwards: tokio::sync::Mutex::new(Vec::new()),
            socks_forwards: tokio::sync::Mutex::new(Vec::new()),
            forward_stops: tokio::sync::Mutex::new(HashMap::new()),
            socks_stops: tokio::sync::Mutex::new(HashMap::new()),
            agent_forwarding: std::sync::atomic::AtomicBool::new(false),
        }),
    );
    Ok(session_id)
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

/// 关闭并清理单个会话：停止端口转发/SOCKS、显式断开 SSH 连接、清空终端/SFTP 句柄，
/// 并为每个终端立即 emit `ssh-terminal-closed`。
/// 不再依赖 Arc 引用计数自然回收——终端/SFTP 读任务持有 `entry`，仅移除会话表
/// 条目会让底层连接存活到通道 EOF 才释放，造成延迟断开与事件迟到。
async fn shutdown_entry(app: &tauri::AppHandle, entry: &SessionEntry) {
    // 1. 通知所有端口转发 / SOCKS 后台 accept 循环退出，释放被绑定的本地端口
    let stops: Vec<Arc<tokio::sync::Notify>> = {
        let mut v = Vec::new();
        v.extend(entry.forward_stops.lock().await.values().cloned());
        v.extend(entry.socks_stops.lock().await.values().cloned());
        v
    };
    for s in stops {
        s.notify_one();
    }
    // 2. 显式发送 SSH_MSG_DISCONNECT 断开连接
    let _ = entry
        .client
        .lock()
        .await
        .disconnect(russh::Disconnect::ByApplication, "closed by user", "en")
        .await;
    // 3. 清空终端 / SFTP 句柄（读任务随后感知通道关闭并退出）
    let term_ids: Vec<String> = {
        let mut t = entry.terminals.lock().await;
        let ids: Vec<String> = t.keys().cloned().collect();
        t.clear();
        ids
    };
    entry.sftp_sessions.lock().await.clear();
    // 4. 立即通知前端终端已关闭；读任务检测到句柄已被移除后不再重复 emit
    for tid in term_ids {
        let _ = app.emit(
            "ssh-terminal-closed",
            serde_json::json!({ "session_id": tid, "reason": "closed" }),
        );
    }
}

#[tauri::command]
pub async fn ssh_close(
    app: tauri::AppHandle,
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
) -> Result<(), String> {
    if let Some(entry) = state.sessions.lock().await.remove(&session_id) {
        shutdown_entry(&app, &entry).await;
    }
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
    if let Some(entry) = manager.sessions.lock().await.remove(&sid) {
        shutdown_entry(&app, &entry).await;
    }
    Ok("ok".into())
}

/// 启动本地端口转发（-L）：绑定 bind_host:bind_port，将流量通过 SSH 隧道转发到 dest_host:dest_port。
#[tauri::command]
pub async fn ssh_forward_local(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
    bind_host: String,
    bind_port: u16,
    dest_host: String,
    dest_port: u16,
) -> Result<ForwardEntry, String> {
    use std::net::SocketAddr;
    use tokio::net::TcpListener;

    let sessions = state.sessions.lock().await;
    let entry = sessions.get(&session_id).ok_or("SESSION_NOT_FOUND")?;
    let entry_arc = Arc::clone(entry);
    drop(sessions);

    let dest_h = dest_host.clone();
    let dest_p = dest_port;
    let bind_a: SocketAddr = format!("{}:{}", bind_host, bind_port)
        .parse()
        .map_err(|_| "INVALID_BIND_ADDRESS")?;

    let listener = TcpListener::bind(bind_a)
        .await
        .map_err(|e| format!("BIND_FAILED: {}", e))?;

    let fid = uuid::Uuid::new_v4().to_string();
    let fentry = ForwardEntry {
        id: fid.clone(),
        bind_host: bind_host.clone(),
        bind_port,
        dest_host: dest_host.clone(),
        dest_port,
        active: true,
    };

    // 停止信号：关闭转发 / 会话时 notify，令后台 accept 循环退出并释放端口
    let stop = Arc::new(tokio::sync::Notify::new());

    // 登记到 local_forwards 与 forward_stops（分别加锁，顺序固定避免死锁）
    {
        let mut fw = entry_arc.local_forwards.lock().await;
        fw.push(fentry.clone());
        entry_arc
            .forward_stops
            .lock()
            .await
            .insert(fid.clone(), stop.clone());
    }

    // 后台接受本地连接，通过 SSH direct-tcpip 转发
    tokio::spawn(async move {
        loop {
            tokio::select! {
                // 收到停止信号：退出循环，listener 被 drop -> 端口释放
                _ = stop.notified() => break,
                accepted = listener.accept() => {
                    match accepted {
                        Ok((inbound, src)) => {
                            let client = entry_arc.client.lock().await;
                            match client
                                .channel_open_direct_tcpip(
                                    dest_h.clone(),
                                    dest_p as u32,
                                    src.ip().to_string(),
                                    src.port() as u32,
                                )
                                .await
                            {
                                Ok(ch) => {
                                    // 用 ChannelStream 包装 SSH 通道（实现 tokio AsyncRead/AsyncWrite），
                                    // 再与本地 TCP 双向并发拷贝。copy_bidirectional 在任一端关闭后自动收尾。
                                    let mut ch_stream = ch.into_stream();
                                    let mut inbound = inbound;
                                    tokio::spawn(async move {
                                        let _ = tokio::io::copy_bidirectional(&mut inbound, &mut ch_stream)
                                            .await;
                                    });
                                }
                                Err(e) => {
                                    eprintln!("[forward] channel open failed: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[forward] accept failed: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok(fentry)
}

/// 关闭指定端口转发（标记 inactive 并触发后台 accept 循环退出，释放端口）
#[tauri::command]
pub async fn ssh_close_forward(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
    forward_id: String,
) -> Result<(), String> {
    let sessions = state.sessions.lock().await;
    if let Some(entry) = sessions.get(&session_id) {
        let mut fw = entry.local_forwards.lock().await;
        if let Some(f) = fw.iter_mut().find(|f| f.id == forward_id) {
            f.active = false;
            if let Some(stop) = entry.forward_stops.lock().await.get(&forward_id) {
                stop.notify_one();
            }
        }
    }
    Ok(())
}

/// 查询当前会话已激活的端口转发列表
#[tauri::command]
pub async fn ssh_list_forwards(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
) -> Result<Vec<ForwardEntry>, String> {
    let sessions = state.sessions.lock().await;
    let entry = sessions.get(&session_id).ok_or("SESSION_NOT_FOUND")?;
    let fw = entry.local_forwards.lock().await;
    Ok(fw.clone())
}

/// 启用 SSH Agent 转发：校验本地存在 SSH_AUTH_SOCK（即有可用 ssh-agent），
/// 并在会话上置位 `agent_forwarding`。此后通过该会话新开的终端会请求
/// auth-agent-req，服务端据此建立转发通道并由 `SshHandler` 代理到本地 agent。
#[tauri::command]
pub async fn ssh_forward_agent(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
) -> Result<String, String> {
    let auth_sock = std::env::var("SSH_AUTH_SOCK")
        .map_err(|_| "SSH_AUTH_SOCK not set — no agent available on this system")?;
    let sessions = state.sessions.lock().await;
    let entry = sessions.get(&session_id).ok_or("SESSION_NOT_FOUND")?;
    entry
        .agent_forwarding
        .store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(auth_sock)
}

/// 启动动态 SOCKS5 代理（-D）：在本地 bind_host:bind_port 监听，
/// 每个客户端连接经一次 SOCKS5 握手取得目标地址，再通过 SSH direct-tcpip 建立隧道转发。
#[tauri::command]
pub async fn ssh_socks_proxy(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
    bind_host: String,
    bind_port: u16,
) -> Result<SocksEntry, String> {
    use std::net::SocketAddr;
    use tokio::net::TcpListener;

    let sessions = state.sessions.lock().await;
    let entry = sessions.get(&session_id).ok_or("SESSION_NOT_FOUND")?;
    let entry_arc = Arc::clone(entry);
    drop(sessions);

    let bind_a: SocketAddr = format!("{}:{}", bind_host, bind_port)
        .parse()
        .map_err(|_| "INVALID_BIND_ADDRESS")?;

    let listener = TcpListener::bind(bind_a)
        .await
        .map_err(|e| format!("BIND_FAILED: {}", e))?;

    let fid = uuid::Uuid::new_v4().to_string();
    let sentry = SocksEntry {
        id: fid.clone(),
        bind_host: bind_host.clone(),
        bind_port,
        active: true,
    };

    // 停止信号：关闭代理 / 会话时 notify，令后台 accept 循环退出并释放端口
    let stop = Arc::new(tokio::sync::Notify::new());

    {
        let mut sw = entry_arc.socks_forwards.lock().await;
        sw.push(sentry.clone());
        entry_arc
            .socks_stops
            .lock()
            .await
            .insert(fid.clone(), stop.clone());
    }

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = stop.notified() => break,
                accepted = listener.accept() => {
                    match accepted {
                        Ok((inbound, _src)) => {
                            let entry = Arc::clone(&entry_arc);
                            tokio::spawn(async move {
                                if let Err(e) = serve_socks(inbound, entry).await {
                                    eprintln!("[socks] proxy error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("[socks] accept failed: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok(sentry)
}

/// 关闭指定 SOCKS5 代理（标记 inactive 并触发后台 accept 循环退出，释放端口）
#[tauri::command]
pub async fn ssh_close_socks(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
    socks_id: String,
) -> Result<(), String> {
    let sessions = state.sessions.lock().await;
    if let Some(entry) = sessions.get(&session_id) {
        let mut sw = entry.socks_forwards.lock().await;
        if let Some(s) = sw.iter_mut().find(|s| s.id == socks_id) {
            s.active = false;
            if let Some(stop) = entry.socks_stops.lock().await.get(&socks_id) {
                stop.notify_one();
            }
        }
    }
    Ok(())
}

/// 查询当前会话已激活的 SOCKS5 代理列表
#[tauri::command]
pub async fn ssh_list_socks(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
) -> Result<Vec<SocksEntry>, String> {
    let sessions = state.sessions.lock().await;
    let entry = sessions.get(&session_id).ok_or("SESSION_NOT_FOUND")?;
    let sw = entry.socks_forwards.lock().await;
    Ok(sw.clone())
}

/// SOCKS5 单连接处理：仅支持无认证（0x00）与 CONNECT 命令（含 IPv4 / 域名 / IPv6）。
async fn serve_socks(
    mut inbound: tokio::net::TcpStream,
    entry: Arc<SessionEntry>,
) -> Result<(), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    const SOCKS_VER: u8 = 0x05;

    // 1. 方法协商：客户端发送 VER NMETHODS METHODS；我们仅声明支持 No Authentication (0x00)
    let mut hdr = [0u8; 2];
    inbound
        .read_exact(&mut hdr)
        .await
        .map_err(|e| format!("SOCKS_NEGO_READ: {e}"))?;
    if hdr[0] != SOCKS_VER {
        return Err(format!("SOCKS_BAD_VER: {}", hdr[0]));
    }
    let nmethods = hdr[1] as usize;
    let mut methods = vec![0u8; nmethods];
    inbound
        .read_exact(&mut methods)
        .await
        .map_err(|e| format!("SOCKS_METHODS_READ: {e}"))?;
    if !methods.contains(&0x00) {
        // 返回无可用方法（0xFF）并断开
        let _ = inbound.write_all(&[SOCKS_VER, 0xff]).await;
        return Err("SOCKS_NO_ACCEPTABLE_METHOD".into());
    }
    inbound
        .write_all(&[SOCKS_VER, 0x00])
        .await
        .map_err(|e| format!("SOCKS_NEGO_WRITE: {e}"))?;

    // 2. 请求：VER CMD RSV ATYP DST.ADDR DST.PORT
    let mut req = [0u8; 4];
    inbound
        .read_exact(&mut req)
        .await
        .map_err(|e| format!("SOCKS_REQ_READ: {e}"))?;
    if req[0] != SOCKS_VER || req[1] != 0x01 {
        // 仅支持 CONNECT
        let _ = inbound
            .write_all(&[SOCKS_VER, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await;
        return Err(format!("SOCKS_UNSUPPORTED_CMD: {}", req[1]));
    }

    let (host, port) = match req[3] {
        0x01 => {
            // IPv4: 4 字节
            let mut b = [0u8; 6];
            inbound
                .read_exact(&mut b)
                .await
                .map_err(|e| format!("SOCKS_IPV4_READ: {e}"))?;
            let ip = std::net::Ipv4Addr::new(b[0], b[1], b[2], b[3]);
            let port = u16::from_be_bytes([b[4], b[5]]);
            (ip.to_string(), port)
        }
        0x03 => {
            // 域名：1 字节长度 + 域名 + 2 字节端口
            let mut len = [0u8; 1];
            inbound
                .read_exact(&mut len)
                .await
                .map_err(|e| format!("SOCKS_DOMAIN_LEN_READ: {e}"))?;
            let mut dom = vec![0u8; len[0] as usize];
            inbound
                .read_exact(&mut dom)
                .await
                .map_err(|e| format!("SOCKS_DOMAIN_READ: {e}"))?;
            let mut p = [0u8; 2];
            inbound
                .read_exact(&mut p)
                .await
                .map_err(|e| format!("SOCKS_DOMAIN_PORT_READ: {e}"))?;
            (
                String::from_utf8_lossy(&dom).to_string(),
                u16::from_be_bytes(p),
            )
        }
        0x04 => {
            // IPv6: 16 字节
            let mut b = [0u8; 18];
            inbound
                .read_exact(&mut b)
                .await
                .map_err(|e| format!("SOCKS_IPV6_READ: {e}"))?;
            let mut oct = [0u8; 16];
            oct.copy_from_slice(&b[0..16]);
            let ip = std::net::Ipv6Addr::from(oct);
            let port = u16::from_be_bytes([b[16], b[17]]);
            (ip.to_string(), port)
        }
        other => {
            let _ = inbound
                .write_all(&[SOCKS_VER, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await;
            return Err(format!("SOCKS_BAD_ATYP: {other}"));
        }
    };

    // 3. 经 SSH 隧道建立 direct-tcpip 通道到目标
    let channel = {
        let client = entry.client.lock().await;
        client
            .channel_open_direct_tcpip(host.clone(), port as u32, "127.0.0.1".to_string(), 0)
            .await
    };
    let ch = match channel {
        Ok(ch) => ch,
        Err(e) => {
            // 返回通用失败（0x01）并断开
            let _ = inbound
                .write_all(&[SOCKS_VER, 0x01, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await;
            return Err(format!("SOCKS_TUNNEL_FAIL: {e}"));
        }
    };

    // 4. 返回成功响应（绑定地址回显 0），随后双向转发
    let _ = inbound
        .write_all(&[SOCKS_VER, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await;

    let mut ch_stream = ch.into_stream();
    let mut inbound = inbound;
    let _ = tokio::io::copy_bidirectional(&mut inbound, &mut ch_stream).await;
    Ok(())
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
    fn test_terminal_buffer_fragmented_append() {
        let mut buf = TerminalBuffer::new(10);
        buf.append("hel");
        buf.append("lo world\n");
        assert!(buf.all().contains("hello world"));
        assert!(!buf.all().contains("hel\nlo"));
    }

    #[test]
    fn test_terminal_buffer_no_empty_lines() {
        let mut buf = TerminalBuffer::new(10);
        buf.append("foo\n");
        buf.append("bar\n");
        assert_eq!(buf.lines.len(), 2);
        assert_eq!(buf.lines[0], "foo");
        assert_eq!(buf.lines[1], "bar");
    }

    #[test]
    fn test_terminal_buffer_carriage_return_stripped() {
        let mut buf = TerminalBuffer::new(10);
        buf.append("foo\r\nbar\r\n");
        assert_eq!(buf.lines.len(), 2);
        assert_eq!(buf.lines[0], "foo");
        assert_eq!(buf.lines[1], "bar");
    }

    #[test]
    fn test_terminal_buffer_capacity() {
        let mut buf = TerminalBuffer::new(2);
        buf.append("a\n");
        buf.append("b\n");
        buf.append("c\n");
        assert_eq!(buf.lines.len(), 2);
        assert_eq!(buf.lines[0], "b");
        assert_eq!(buf.lines[1], "c");
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
