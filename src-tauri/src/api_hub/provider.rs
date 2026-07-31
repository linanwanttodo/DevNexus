use super::types::{ApiProtocol, AppState, Provider};

/// 添加 Provider（async，使用 tokio locks）
pub async fn add_provider(state: &AppState, provider: Provider) -> Result<(), String> {
    // 如果 ID 为空，生成新 UUID；否则尊重调用方 ID
    let provider = if provider.id.is_empty() {
        Provider {
            id: uuid::Uuid::new_v4().to_string(),
            ..provider
        }
    } else {
        provider
    };

    // 1) 内存重名检查（先于 DB 写入，保证失败无副作用）
    {
        let providers = state.providers.read().await;
        if providers
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&provider.name))
        {
            return Err(format!("Provider '{}' already exists", provider.name));
        }
    }

    // 2) 持久化到 SQLite（失败则向上传播错误）
    {
        let db = state.db.lock().await;
        if let Some(ref conn) = *db {
            conn.execute(
                "INSERT INTO providers (id, name, protocol, base_url, api_key, models, enabled, created_at, model_aliases, model_context_lengths)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                    serde_json::to_string(&provider.model_context_lengths).unwrap_or_else(|_| "{}".to_string()),
                ],
            )
            .map_err(|e| format!("Database error: {}", e))?;
        }
    }

    // 3) DB 成功后更新内存
    let mut providers = state.providers.write().await;
    providers.push(provider);
    Ok(())
}

/// 删除 Provider
pub async fn delete_provider(state: &AppState, id: &str) -> Result<(), String> {
    // 先删除数据库记录
    {
        let db = state.db.lock().await;
        if let Some(ref conn) = *db {
            conn.execute("DELETE FROM providers WHERE id = ?1", rusqlite::params![id])
                .map_err(|e| format!("Database error: {}", e))?;
        }
    }

    // 更新内存
    let mut providers = state.providers.write().await;
    providers.retain(|p| p.id != id);
    Ok(())
}

/// 更新 Provider（先写 DB，成功后更新内存）
pub async fn update_provider(state: &AppState, id: &str, provider: Provider) -> Result<(), String> {
    // 前端列表中的 api_key 已脱敏；若传回的仍是掩码，则保留原 key
    let provider = if provider.api_key.contains("••••") {
        let providers = state.providers.read().await;
        let old_key = providers
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.api_key.clone())
            .unwrap_or_default();
        Provider {
            api_key: old_key,
            ..provider
        }
    } else {
        provider
    };

    // 先持久化到数据库
    {
        let db = state.db.lock().await;
        if let Some(ref conn) = *db {
            conn.execute(
                "UPDATE providers SET name=?1, protocol=?2, base_url=?3, api_key=?4, models=?5, enabled=?6, model_aliases=?7, model_context_lengths=?8 WHERE id=?9",
                rusqlite::params![
                    provider.name,
                    provider.protocol.as_str(),
                    provider.base_url,
                    provider.api_key,
                    serde_json::to_string(&provider.models).unwrap_or_default(),
                    provider.enabled as i32,
                    serde_json::to_string(&provider.model_aliases).unwrap_or_else(|_| "{}".to_string()),
                    serde_json::to_string(&provider.model_context_lengths).unwrap_or_else(|_| "{}".to_string()),
                    id,
                ],
            )
            .map_err(|e| format!("Database error: {}", e))?;
        }
    }

    // DB 成功后更新内存
    let mut providers = state.providers.write().await;
    if let Some(p) = providers.iter_mut().find(|p| p.id == id) {
        p.name = provider.name;
        p.protocol = provider.protocol;
        p.base_url = provider.base_url;
        p.api_key = provider.api_key;
        p.models = provider.models;
        p.model_aliases = provider.model_aliases;
        p.model_context_lengths = provider.model_context_lengths;
        p.enabled = provider.enabled;
    }

    Ok(())
}

/// 从 SQLite 加载已保存的 Provider（启动时调用，同步上下文）
pub fn load_providers_from_db_sync(conn: &rusqlite::Connection) -> Vec<Provider> {
    let mut stmt = match conn.prepare(
        "SELECT id, name, protocol, base_url, api_key, models, model_aliases, enabled, created_at, model_context_lengths FROM providers",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stmt.query_map([], |row| {
        let protocol_str: String = row.get(2)?;
        let protocol =
            ApiProtocol::from_protocol_str(&protocol_str).unwrap_or(ApiProtocol::OpenAIChat);
        let models_str: String = row.get(5)?;
        let models: Vec<String> = serde_json::from_str(&models_str).unwrap_or_default();
        let aliases_str: String = row.get(6).unwrap_or_else(|_| "{}".to_string());
        let model_aliases: std::collections::HashMap<String, String> =
            serde_json::from_str(&aliases_str).unwrap_or_default();
        let ctx_str: String = row.get(9).unwrap_or_else(|_| "{}".to_string());
        let model_context_lengths: std::collections::HashMap<String, u64> =
            serde_json::from_str(&ctx_str).unwrap_or_default();
        Ok(Provider {
            id: row.get(0)?,
            name: row.get(1)?,
            protocol,
            base_url: row.get(3)?,
            api_key: row.get(4)?,
            models,
            model_aliases,
            model_context_lengths,
            enabled: row.get::<_, i32>(7)? != 0,
            created_at: row.get(8)?,
        })
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// 初始化数据库表
pub fn init_db_sync(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute_batch(
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
    )
    .map_err(|e| format!("Failed to create providers table: {}", e))?;

    // 旧表迁移：若存在 provider_type 列但无 protocol 列，则合并
    migrate_provider_type_to_protocol(conn);

    // 新增 model_context_lengths 列（若不存在）
    let _ = conn.execute(
        "ALTER TABLE providers ADD COLUMN model_context_lengths TEXT NOT NULL DEFAULT '{}'",
        [],
    );

    // C2: Provider 名称唯一约束。先清理历史重名数据（每组保留 id 最小的一条），
    // 再建唯一索引（幂等）。DELETE 失败（如表不存在）则跳过，由建索引兜底报错。
    let _ = conn.execute(
        "DELETE FROM providers WHERE id NOT IN (SELECT MIN(id) FROM providers GROUP BY lower(name))",
        [],
    );
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_name_unique ON providers(name)",
        [],
    )
    .map_err(|e| format!("Failed to create unique index on providers.name: {}", e))?;

    conn.execute_batch(
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
        CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON request_logs(timestamp);
        CREATE INDEX IF NOT EXISTS idx_logs_model ON request_logs(model);",
    )
    .map_err(|e| format!("Failed to create request_logs table: {}", e))?;

    Ok(())
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

    // 新增 protocol 列
    let _ = conn.execute(
        "ALTER TABLE providers ADD COLUMN protocol TEXT NOT NULL DEFAULT 'openai_chat'",
        [],
    );

    if has_old_type {
        // 依据旧字段推导新协议
        let _ = conn.execute(
            "UPDATE providers SET protocol = CASE
                WHEN provider_type = 'anthropic' THEN 'anthropic'
                WHEN provider_type = 'openai' AND api_format = 'responses' THEN 'openai_responses'
                ELSE 'openai_chat'
            END",
            [],
        );
    }

    // 删除不再支持的协议的 Provider（Gemini/Ollama）
    let _ = conn.execute(
        "DELETE FROM providers WHERE protocol NOT IN ('openai_chat', 'openai_responses', 'anthropic')",
        [],
    );
}
