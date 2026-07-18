use super::types::{ApiProtocol, AppState, Provider};

/// 添加 Provider
pub fn add_provider(state: &AppState, provider: Provider) -> Result<(), String> {
    let mut providers = state.providers.write().map_err(|e| e.to_string())?;

    // 检查重名
    if providers.iter().any(|p| p.name == provider.name) {
        return Err(format!("Provider '{}' already exists", provider.name));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let provider = Provider { id, ..provider };

    // 持久化到 SQLite
    if let Ok(db) = state.db.lock() {
        if let Some(ref conn) = *db {
            let _ = conn.execute(
                "INSERT INTO providers (id, name, protocol, base_url, api_key, models, enabled, created_at, model_aliases)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    provider.id,
                    provider.name,
                    provider.protocol.as_str(),
                    provider.base_url,
                    provider.api_key,
                    serde_json::to_string(&provider.models).unwrap_or_default(),
                    provider.enabled as i32,
                    provider.created_at,
                    serde_json::to_string(&provider.model_aliases).unwrap_or_else(|_| "{}".to_string()),
                ],
            );
        }
    }

    providers.push(provider);
    Ok(())
}

/// 删除 Provider
pub fn delete_provider(state: &AppState, id: &str) -> Result<(), String> {
    let mut providers = state.providers.write().map_err(|e| e.to_string())?;
    providers.retain(|p| p.id != id);

    if let Ok(db) = state.db.lock() {
        if let Some(ref conn) = *db {
            let _ = conn.execute("DELETE FROM providers WHERE id = ?1", rusqlite::params![id]);
        }
    }

    Ok(())
}

/// 更新 Provider
pub fn update_provider(state: &AppState, id: &str, provider: Provider) -> Result<(), String> {
    let mut providers = state.providers.write().map_err(|e| e.to_string())?;

    if let Some(p) = providers.iter_mut().find(|p| p.id == id) {
        p.name = provider.name.clone();
        p.protocol = provider.protocol;
        p.base_url = provider.base_url.clone();
        p.api_key = provider.api_key.clone();
        p.models = provider.models.clone();
        p.model_aliases = provider.model_aliases.clone();
        p.enabled = provider.enabled;
    }

    // 持久化
    if let Ok(db) = state.db.lock() {
        if let Some(ref conn) = *db {
            let _ = conn.execute(
                "UPDATE providers SET name=?1, protocol=?2, base_url=?3, api_key=?4, models=?5, enabled=?6, model_aliases=?7 WHERE id=?8",
                rusqlite::params![
                    provider.name,
                    provider.protocol.as_str(),
                    provider.base_url,
                    provider.api_key,
                    serde_json::to_string(&provider.models).unwrap_or_default(),
                    provider.enabled as i32,
                    serde_json::to_string(&provider.model_aliases).unwrap_or_else(|_| "{}".to_string()),
                    id,
                ],
            );
        }
    }

    Ok(())
}

/// 从 SQLite 加载已保存的 Provider
pub fn load_providers_from_db(state: &AppState) {
    if let Ok(db) = state.db.lock() {
        if let Some(ref conn) = *db {
            let mut stmt = match conn.prepare(
                "SELECT id, name, protocol, base_url, api_key, models, model_aliases, enabled, created_at FROM providers"
            ) {
                Ok(s) => s,
                Err(_) => return,
            };

            let rows: Vec<Provider> = stmt
                .query_map([], |row| {
                    let protocol_str: String = row.get(2)?;
                    let protocol = ApiProtocol::from_str(&protocol_str)
                        .unwrap_or(ApiProtocol::OpenAIChat);
                    let models_str: String = row.get(5)?;
                    let models: Vec<String> =
                        serde_json::from_str(&models_str).unwrap_or_default();
                    let aliases_str: String = row.get(6).unwrap_or_else(|_| "{}".to_string());
                    let model_aliases: std::collections::HashMap<String, String> =
                        serde_json::from_str(&aliases_str).unwrap_or_default();
                    Ok(Provider {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        protocol,
                        base_url: row.get(3)?,
                        api_key: row.get(4)?,
                        models,
                        model_aliases,
                        enabled: row.get::<_, i32>(7)? != 0,
                        created_at: row.get(8)?,
                    })
                })
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default();

            if let Ok(mut providers) = state.providers.write() {
                *providers = rows;
            }
        }
    }
}

/// 初始化数据库表，并把旧的 (provider_type + api_format) 合并迁移到 protocol
pub fn init_db(state: &AppState) {
    if let Ok(db) = state.db.lock() {
        if let Some(ref conn) = *db {
            // 先确保表存在（新安装）
            let _ = conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS providers (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    protocol TEXT NOT NULL DEFAULT 'openai_chat',
                    base_url TEXT NOT NULL,
                    api_key TEXT NOT NULL,
                    models TEXT NOT NULL DEFAULT '[]',
                    enabled INTEGER NOT NULL DEFAULT 1,
                    created_at INTEGER NOT NULL,
                    model_aliases TEXT NOT NULL DEFAULT '{}'
                );",
            );

            // 旧表迁移：若存在 provider_type 列但无 protocol 列，则合并
            migrate_provider_type_to_protocol(conn);

            // 确保 request_logs 表存在
            let _ = conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS request_logs (
                    id TEXT PRIMARY KEY,
                    provider_id TEXT,
                    provider_name TEXT,
                    model TEXT,
                    request_model TEXT,
                    input_tokens INTEGER DEFAULT 0,
                    output_tokens INTEGER DEFAULT 0,
                    latency_ms INTEGER DEFAULT 0,
                    status_code INTEGER DEFAULT 0,
                    error_message TEXT,
                    timestamp INTEGER NOT NULL,
                    is_streaming INTEGER DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON request_logs(timestamp);",
            );
        }
    }
}

/// 把旧 schema（provider_type + api_format 两列）合并为单一 protocol 列
fn migrate_provider_type_to_protocol(conn: &rusqlite::Connection) {
    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(providers)")
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(1))
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let has_protocol = columns.iter().any(|c| c == "protocol");
    let has_old_type = columns.iter().any(|c| c == "provider_type");

    if has_protocol {
        return;
    }

    // 新增 protocol 列（带默认值，兼容已有行）
    let _ = conn.execute(
        "ALTER TABLE providers ADD COLUMN protocol TEXT NOT NULL DEFAULT 'openai_chat'",
        [],
    );

    if has_old_type {
        // 依据旧字段推导新协议：OpenAI 再按 api_format 区分 chat / responses
        let _ = conn.execute(
            "UPDATE providers SET protocol = CASE
                WHEN provider_type = 'anthropic' THEN 'anthropic'
                WHEN provider_type = 'gemini' THEN 'gemini'
                WHEN provider_type = 'ollama' THEN 'ollama'
                WHEN provider_type = 'openai' AND api_format = 'responses' THEN 'openai_responses'
                ELSE 'openai_chat'
            END",
            [],
        );
    }
}
