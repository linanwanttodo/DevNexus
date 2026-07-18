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

/// 初始化 API Hub：创建共享状态
pub fn init(data_dir: &std::path::Path) -> AppState {
    // 初始化 SQLite
    let db_path = data_dir.join("api_hub.db");
    let conn = rusqlite::Connection::open(&db_path).ok();
    let db = Arc::new(std::sync::Mutex::new(conn));

    let state = AppState {
        providers: Arc::new(std::sync::RwLock::new(Vec::new())),
        request_logs: Arc::new(std::sync::RwLock::new(Vec::new())),
        db: db.clone(),
    };

    // 初始化数据库表
    provider::init_db(&state);

    // 从数据库加载已保存的 Provider
    provider::load_providers_from_db(&state);

    // 如果没有 Provider，添加默认的 Ollama（自动检测）
    if state.providers.read().ok().map(|p| p.is_empty()).unwrap_or(true) {
        let ollama_running = std::net::TcpStream::connect_timeout(
            &"127.0.0.1:11434".parse().unwrap(),
            std::time::Duration::from_millis(500),
        )
        .is_ok();

        if ollama_running {
            let _ = provider::add_provider(&state, types::Provider {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Ollama (Local)".to_string(),
                protocol: types::ApiProtocol::Ollama,
                base_url: "http://localhost:11434".to_string(),
                api_key: String::new(),
                models: vec![
                    "llama3.2".to_string(),
                    "qwen2.5".to_string(),
                    "nomic-embed-text".to_string(),
                ],
                model_aliases: std::collections::HashMap::new(),
                enabled: true,
                created_at: chrono::Utc::now().timestamp(),
            });
            println!("[API Hub] Auto-detected Ollama running at localhost:11434");
        }
    }

    state
}

/// 启动 API Hub HTTP 服务（在 Tauri 的异步运行时中运行）
pub async fn start(state: Arc<AppState>) {
    server::start_server(state).await;
}

#[cfg(test)]
mod e2e_tests;
