use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub mod commands;
pub mod fetch_models;
pub mod forwarder;
pub mod provider;
pub mod router;
pub mod server;
pub mod transform;
pub mod types;
pub mod usage;

use types::AppState;

/// 初始化 API Hub：创建共享状态（同步调用，在应用启动时执行）
pub fn init(data_dir: &std::path::Path) -> AppState {
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
        .map(|c| provider::load_providers_from_db_sync(c))
        .unwrap_or_default();

    // 清理过期日志，并恢复最近日志到内存（重启后统计/日志不丢失）
    let request_logs = conn
        .as_ref()
        .map(|c| {
            usage::cleanup_old_logs_sync(c);
            usage::load_recent_logs_sync(c)
        })
        .unwrap_or_default();

    // 创建全局 HTTP Client（复用连接池）
    let http_client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(60))
        .pool_max_idle_per_host(10)
        .build()
        .expect("Failed to create HTTP client");

    AppState {
        providers: Arc::new(tokio::sync::RwLock::new(providers)),
        request_logs: Arc::new(tokio::sync::RwLock::new(request_logs)),
        db: Arc::new(tokio::sync::Mutex::new(conn)),
        http_client,
        running: Arc::new(AtomicBool::new(false)),
    }
}

/// 启动 API Hub HTTP 服务（在 Tauri 的异步运行时中运行）
pub async fn start(state: Arc<AppState>) {
    server::start_server(state).await;
}

#[cfg(test)]
mod e2e_tests;
