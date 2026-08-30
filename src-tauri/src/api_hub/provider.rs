use super::crypto::ApiKeyCipher;
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

    // 2) 持久化到 SQLite（失败则向上传播错误）；api_key 存储边界加密
    // 先在锁外完成加密，避免持锁期间做加解密阻塞
    let encrypted_key = state.api_key_cipher.encrypt(&provider.api_key)?;
    let pid = provider.id.clone();
    let pname = provider.name.clone();
    let pproto = provider.protocol.as_str().to_string();
    let pbase = provider.base_url.clone();
    let pmodels = serde_json::to_string(&provider.models).unwrap_or_default();
    let penabled = provider.enabled as i32;
    let pcreated = provider.created_at;
    let paliases =
        serde_json::to_string(&provider.model_aliases).unwrap_or_else(|_| "{}".to_string());
    let pctx =
        serde_json::to_string(&provider.model_context_lengths).unwrap_or_else(|_| "{}".to_string());
    {
        let db = state.db.lock().await;
        if let Some(ref conn) = *db {
            conn.execute(
                "INSERT INTO providers (id, name, protocol, base_url, api_key, models, enabled, created_at, model_aliases, model_context_lengths)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![pid, pname, pproto, pbase, encrypted_key, pmodels, penabled, pcreated, paliases, pctx],
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
///
/// api_key 传回空串或掩码（`••••`，来自前端脱敏列表）时保留原 key（C6）；
/// id 不存在时返回 Err（DB 影响行数为 0）。
pub async fn update_provider(state: &AppState, id: &str, provider: Provider) -> Result<(), String> {
    // 前端列表中的 api_key 已脱敏；若传回的是空串或掩码，则保留原 key
    let keep_old_key = provider.api_key.is_empty() || provider.api_key.contains("••••");
    let api_key = if keep_old_key {
        // 优先从内存取原 key（明文）；内存缺失（异常场景）时回退查询 DB（存储为密文，需解密还原）
        let mem_key = {
            let providers = state.providers.read().await;
            providers
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.api_key.clone())
        };
        match mem_key {
            Some(k) => k,
            None => {
                let db = state.db.lock().await;
                db.as_ref()
                    .and_then(|conn| {
                        conn.query_row(
                            "SELECT api_key FROM providers WHERE id = ?1",
                            rusqlite::params![id],
                            |row| row.get::<_, String>(0),
                        )
                        .ok()
                    })
                    .map(|stored| state.api_key_cipher.decrypt(&stored))
                    .unwrap_or_default()
            }
        }
    } else {
        provider.api_key
    };

    let provider = Provider {
        api_key,
        ..provider
    };

    // 先在锁外完成加密，避免持锁阻塞
    let pname = provider.name.clone();
    let pproto = provider.protocol.as_str().to_string();
    let pbase = provider.base_url.clone();
    let pm_api_key = provider.api_key.clone();
    let pmodels = serde_json::to_string(&provider.models).unwrap_or_default();
    let penabled = provider.enabled as i32;
    let paliases =
        serde_json::to_string(&provider.model_aliases).unwrap_or_else(|_| "{}".to_string());
    let pctx =
        serde_json::to_string(&provider.model_context_lengths).unwrap_or_else(|_| "{}".to_string());

    // 先持久化到数据库；影响行数为 0 说明 id 不存在；api_key 存储边界加密
    {
        let db = state.db.lock().await;
        match *db {
            Some(ref conn) => {
                let encrypted_key = state.api_key_cipher.encrypt(&pm_api_key)?;
                let affected = conn
                    .execute(
                        "UPDATE providers SET name=?1, protocol=?2, base_url=?3, api_key=?4, models=?5, enabled=?6, model_aliases=?7, model_context_lengths=?8 WHERE id=?9",
                        rusqlite::params![pname, pproto, pbase, encrypted_key, pmodels, penabled, paliases, pctx, id],
                    )
                    .map_err(|e| format!("Database error: {}", e))?;
                if affected == 0 {
                    return Err(format!("Provider not found: {}", id));
                }
            }
            None => {
                // DB 不可用（降级模式）：仅凭内存判断存在性，避免静默成功
                let exists = state.providers.read().await.iter().any(|p| p.id == id);
                if !exists {
                    return Err(format!("Provider not found: {}", id));
                }
            }
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

/// 从 SQLite 加载已保存的 Provider（启动时调用，同步上下文）；api_key 存储为密文，解密还原为明文
pub fn load_providers_from_db_sync(
    conn: &rusqlite::Connection,
    cipher: &ApiKeyCipher,
) -> Vec<Provider> {
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
        let stored_key: String = row.get(4)?;
        let api_key = cipher.decrypt(&stored_key);
        // 存储为密文但解密失败（篡改/密钥不匹配）则跳过该 Provider 并告警，避免加载空 key 的无效配置
        if stored_key.starts_with("enc1:") && api_key.is_empty() && !stored_key.is_empty() {
            let provider_name = row.get::<_, String>(1).unwrap_or_default();
            tracing::error!(
                provider = %provider_name,
                "[API Hub] Failed to decrypt API key for provider, skipping"
            );
            return Err(rusqlite::Error::InvalidParameterName(
                "decrypt_failed".to_string(),
            ));
        }
        Ok(Provider {
            id: row.get(0)?,
            name: row.get(1)?,
            protocol,
            base_url: row.get(3)?,
            api_key,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_conn() -> rusqlite::Connection {
        rusqlite::Connection::open_in_memory().unwrap()
    }

    #[test]
    fn test_init_db_sync_creates_tables() {
        let conn = mem_conn();
        init_db_sync(&conn).expect("init db");
        // 表存在
        let providers: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='providers'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(providers, 1);
        let logs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='request_logs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(logs, 1);
    }

    #[test]
    fn test_init_db_sync_is_idempotent() {
        let conn = mem_conn();
        init_db_sync(&conn).expect("first init");
        init_db_sync(&conn).expect("second init");
        init_db_sync(&conn).expect("third init");
    }

    #[test]
    fn test_provider_name_unique_index() {
        let conn = mem_conn();
        init_db_sync(&conn).unwrap();

        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO providers (id, name, protocol, base_url, api_key, created_at)
             VALUES ('a', 'same-name', 'openai_chat', 'http://x', '', ?1)",
            rusqlite::params![now],
        )
        .unwrap();
        // 同名第二条插入应失败（唯一索引）
        let res = conn.execute(
            "INSERT INTO providers (id, name, protocol, base_url, api_key, created_at)
             VALUES ('b', 'same-name', 'openai_chat', 'http://y', '', ?1)",
            rusqlite::params![now],
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_provider_insert_and_load_roundtrip() {
        use crate::api_hub::crypto::ApiKeyCipher;

        let conn = mem_conn();
        init_db_sync(&conn).unwrap();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO providers (id, name, protocol, base_url, api_key, models, created_at)
             VALUES ('p1', 'My Provider', 'anthropic', 'https://api.anthropic.com', 'secret-key',
                     '[\"claude-3\"]', ?1)",
            rusqlite::params![now],
        )
        .unwrap();

        // 用明文降级 cipher（无钥匙串/文件时 enabled=false，decrypt 原样返回）
        let tmp =
            std::env::temp_dir().join(format!("devnexus_provider_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let cipher = ApiKeyCipher::load_or_create(&tmp);

        let providers = load_providers_from_db_sync(&conn, &cipher);
        assert_eq!(providers.len(), 1);
        let p = &providers[0];
        assert_eq!(p.id, "p1");
        assert_eq!(p.name, "My Provider");
        assert_eq!(p.models, vec!["claude-3".to_string()]);
        assert_eq!(p.api_key, "secret-key");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_migrate_old_schema_provider_type() {
        let conn = mem_conn();
        // 手工创建旧 schema（provider_type + api_format，无 protocol 列）
        conn.execute_batch(
            "CREATE TABLE providers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                provider_type TEXT NOT NULL DEFAULT 'openai',
                api_format TEXT NOT NULL DEFAULT 'chat',
                base_url TEXT NOT NULL,
                api_key TEXT NOT NULL,
                models TEXT NOT NULL DEFAULT '[]',
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL
            );
            INSERT INTO providers (id, name, provider_type, api_format, base_url, api_key, created_at)
            VALUES ('m1', 'Legacy Anthropic', 'anthropic', 'chat', 'http://a', '', 1);
            INSERT INTO providers (id, name, provider_type, api_format, base_url, api_key, created_at)
            VALUES ('m2', 'Legacy Responses', 'openai', 'responses', 'http://b', '', 1);
            INSERT INTO providers (id, name, provider_type, api_format, base_url, api_key, created_at)
            VALUES ('m3', 'Legacy Chat', 'openai', 'chat', 'http://c', '', 1);",
        )
        .unwrap();

        // 迁移后应新增 protocol 列并按旧字段推导
        migrate_provider_type_to_protocol(&conn);

        let rows: Vec<(String, String)> = conn
            .prepare("SELECT name, protocol FROM providers ORDER BY name")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0],
            ("Legacy Anthropic".to_string(), "anthropic".to_string())
        );
        assert_eq!(
            rows[1],
            ("Legacy Chat".to_string(), "openai_chat".to_string())
        );
        assert_eq!(
            rows[2],
            (
                "Legacy Responses".to_string(),
                "openai_responses".to_string()
            )
        );
    }

    #[test]
    fn test_migrate_new_schema_noop() {
        let conn = mem_conn();
        init_db_sync(&conn).unwrap();
        // 已有 protocol 列时迁移不做任何事
        migrate_provider_type_to_protocol(&conn);
        migrate_provider_type_to_protocol(&conn);
    }
}
