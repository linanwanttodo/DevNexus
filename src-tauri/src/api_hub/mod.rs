use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub mod commands;
pub mod crypto;
pub mod fetch_models;
pub mod forwarder;
pub mod provider;
pub mod router;
pub mod server;
pub mod transform;
pub mod types;
pub mod usage;

use types::AppState;

/// 访问令牌文件（0600 权限，供外部客户端长期使用）
const TOKEN_FILE: &str = "api_hub_token";

/// 加载或创建本地服务访问令牌。
/// 首次运行生成 48 位随机字符并写入 data_dir（0600）；之后复用，
/// 保证用户配置的 IDE 客户端 token 在重启后依然有效。
fn load_or_create_token(data_dir: &std::path::Path) -> String {
    let token_path = data_dir.join(TOKEN_FILE);
    if let Ok(t) = std::fs::read_to_string(&token_path) {
        let t = t.trim().to_string();
        if t.len() >= 32 && !t.contains(char::is_whitespace) {
            return t;
        }
    }

    use rand::distributions::Alphanumeric;
    use rand::Rng;
    let token: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();

    if std::fs::create_dir_all(data_dir).is_ok() {
        if std::fs::write(&token_path, &token).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&token_path) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o600);
                    let _ = std::fs::set_permissions(&token_path, perms);
                }
            }
            return token;
        }
        eprintln!(
            "[API Hub] WARNING: failed to persist auth token to {}; using in-memory token.",
            token_path.display()
        );
    }
    token
}

/// 初始化 API Hub：创建共享状态（同步调用，在应用启动时执行）
pub fn init(data_dir: &std::path::Path) -> AppState {
    // API key 加密器（OS keyring → data_dir 文件兜底 → 明文降级）
    let api_key_cipher = Arc::new(crypto::ApiKeyCipher::load_or_create(data_dir));

    // 本地服务访问令牌（H1 安全修复）
    let auth_token = load_or_create_token(data_dir);

    // 初始化 SQLite
    let db_path = data_dir.join("api_hub.db");
    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!(
                "[API Hub] Failed to open database {:?}: {} (providers will not persist)",
                db_path, e
            );
            None
        }
    };

    // 初始化数据库表
    if let Some(ref c) = conn {
        if let Err(e) = provider::init_db_sync(c) {
            eprintln!("[API Hub] Database init error: {}", e);
        }
    }

    // 从数据库加载已保存的 Provider
    let providers = conn
        .as_ref()
        .map(|c| provider::load_providers_from_db_sync(c, &api_key_cipher))
        .unwrap_or_default();

    // 清理过期日志，并恢复最近日志到内存（重启后统计/日志不丢失）
    let request_logs = conn
        .as_ref()
        .map(|c| {
            usage::cleanup_old_logs_sync(c);
            usage::load_recent_logs_sync(c)
        })
        .unwrap_or_default();

    // 创建全局 HTTP Client（复用连接池）；构建失败时降级到默认 Client，避免启动 panic
    let http_client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
        .pool_max_idle_per_host(10)
        .build()
        .unwrap_or_else(|e| {
            eprintln!(
                "[API Hub] Failed to build HTTP client, using default: {}",
                e
            );
            reqwest::Client::new()
        });

    AppState {
        providers: Arc::new(tokio::sync::RwLock::new(providers)),
        request_logs: Arc::new(tokio::sync::RwLock::new(request_logs)),
        db: Arc::new(tokio::sync::Mutex::new(conn)),
        http_client,
        running: Arc::new(AtomicBool::new(false)),
        api_key_cipher,
        auth_token,
    }
}

/// 启动 API Hub HTTP 服务（在 Tauri 的异步运行时中运行）
pub async fn start(state: Arc<AppState>) {
    server::start_server(state).await;
}

#[cfg(test)]
mod e2e_tests;
