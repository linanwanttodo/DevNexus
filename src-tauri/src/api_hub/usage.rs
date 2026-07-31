use super::types::{AppState, RequestLog};
use std::collections::VecDeque;

const MAX_LOGS_IN_MEMORY: usize = 1000;
/// 数据库中日志保留天数（超过即清理，防止无限增长）
const LOG_RETENTION_DAYS: i64 = 30;
/// 数据库中日志最大条数（超出时删除最旧的）
const MAX_LOGS_IN_DB: i64 = 50_000;
/// 错误信息最大保留长度（防止超大响应体占用内存/数据库）
const MAX_ERROR_LEN: usize = 500;

/// 记录一次请求到日志（async 版本）
pub async fn log_request(state: &AppState, mut log: RequestLog) {
    // 截断超长错误信息，避免大响应体撑爆内存与 DB
    if let Some(ref mut err) = log.error_message {
        if err.len() > MAX_ERROR_LEN {
            let mut cut = MAX_ERROR_LEN;
            while !err.is_char_boundary(cut) {
                cut -= 1;
            }
            err.truncate(cut);
            err.push_str("…");
        }
    }

    // 内存中保留最近 MAX_LOGS_IN_MEMORY 条（VecDeque O(1) 移除）
    {
        let mut logs = state.request_logs.write().await;
        if logs.len() >= MAX_LOGS_IN_MEMORY {
            logs.pop_front();
        }
        logs.push_back(log.clone());
    }

    // 异步持久化到 SQLite（await 确保 INSERT 提交后再返回，避免流式结束后
    // update_log_tokens 的 UPDATE 先于 INSERT 执行导致 token 统计丢失）
    let db = state.db.clone();
    let _ = tokio::task::spawn_blocking(move || {
        let db_guard = db.blocking_lock();
        if let Some(ref conn) = *db_guard {
            if let Err(e) = conn.execute(
                "INSERT INTO request_logs (id, provider_id, provider_name, model, request_model,
                 input_tokens, output_tokens, latency_ms, status_code, error_message, timestamp, is_streaming)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                rusqlite::params![
                    log.id,
                    log.provider_id,
                    log.provider_name,
                    log.model,
                    log.request_model,
                    log.input_tokens as i64,
                    log.output_tokens as i64,
                    log.latency_ms as i64,
                    log.status_code,
                    log.error_message,
                    log.timestamp,
                    log.is_streaming as i32,
                ],
            ) {
                eprintln!("[API Hub] Failed to insert request log: {}", e);
            }
        }
    })
    .await;
}

/// 回填流式请求的 token 用量（流结束后调用）
pub async fn update_log_tokens(state: &AppState, log_id: &str, input: u64, output: u64) {
    if input == 0 && output == 0 {
        return;
    }

    // 更新内存中的日志条目
    {
        let mut logs = state.request_logs.write().await;
        if let Some(entry) = logs.iter_mut().rev().find(|l| l.id == log_id) {
            entry.input_tokens = input;
            entry.output_tokens = output;
        }
    }

    // 更新数据库（await 确保 UPDATE 提交后再返回，且此时 INSERT 已提交，保证匹配到行）
    let db = state.db.clone();
    let id = log_id.to_string();
    let _ = tokio::task::spawn_blocking(move || {
        let db_guard = db.blocking_lock();
        if let Some(ref conn) = *db_guard {
            if let Err(e) = conn.execute(
                "UPDATE request_logs SET input_tokens = ?1, output_tokens = ?2 WHERE id = ?3",
                rusqlite::params![input as i64, output as i64, id],
            ) {
                eprintln!("[API Hub] Failed to update request log tokens: {}", e);
            }
        }
    })
    .await;
}

/// 获取请求日志列表（优先查 SQLite，DB 不可用时回退内存）
pub async fn get_logs(state: &AppState, limit: usize, offset: usize) -> Vec<RequestLog> {
    let db = state.db.clone();
    let db_result = tokio::task::spawn_blocking(move || {
        let db_guard = db.blocking_lock();
        let conn = (*db_guard).as_ref()?;
        query_logs_sync(conn, limit, offset)
    })
    .await
    .ok()
    .flatten();

    if let Some(logs) = db_result {
        return logs;
    }

    // 回退：内存日志（从尾部最新开始）
    let logs = state.request_logs.read().await;
    logs.iter().rev().skip(offset).take(limit).cloned().collect()
}

/// 获取用量统计数据（优先 SQLite 聚合查询，DB 不可用时回退内存聚合）
pub async fn get_usage_stats(state: &AppState) -> UsageStats {
    let db = state.db.clone();
    let db_result = tokio::task::spawn_blocking(move || {
        let db_guard = db.blocking_lock();
        let conn = (*db_guard).as_ref()?;
        aggregate_stats_sync(conn)
    })
    .await
    .ok()
    .flatten();

    if let Some(stats) = db_result {
        return stats;
    }

    // 回退：基于内存日志聚合
    let logs = state.request_logs.read().await;
    stats_from_iter(logs.iter())
}

// ── SQLite 查询（同步，供 spawn_blocking 调用） ────────────────

fn row_to_log(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestLog> {
    Ok(RequestLog {
        id: row.get(0)?,
        provider_id: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        provider_name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        model: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        request_model: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        input_tokens: row.get::<_, i64>(5)?.max(0) as u64,
        output_tokens: row.get::<_, i64>(6)?.max(0) as u64,
        latency_ms: row.get::<_, i64>(7)?.max(0) as u64,
        status_code: row.get::<_, i64>(8)?.clamp(0, u16::MAX as i64) as u16,
        error_message: row.get(9)?,
        timestamp: row.get(10)?,
        is_streaming: row.get::<_, i64>(11)? != 0,
    })
}

const LOG_COLUMNS: &str = "id, provider_id, provider_name, model, request_model, \
    input_tokens, output_tokens, latency_ms, status_code, error_message, timestamp, is_streaming";

fn query_logs_sync(
    conn: &rusqlite::Connection,
    limit: usize,
    offset: usize,
) -> Option<Vec<RequestLog>> {
    let sql = format!(
        "SELECT {} FROM request_logs ORDER BY timestamp DESC, rowid DESC LIMIT ?1 OFFSET ?2",
        LOG_COLUMNS
    );
    let mut stmt = conn.prepare_cached(&sql).ok()?;
    let rows = stmt
        .query_map(
            rusqlite::params![limit as i64, offset as i64],
            row_to_log,
        )
        .ok()?;
    Some(rows.filter_map(|r| r.ok()).collect())
}

fn aggregate_stats_sync(conn: &rusqlite::Connection) -> Option<UsageStats> {
    let mut stats = UsageStats::default();

    // 总量聚合
    {
        let mut stmt = conn
            .prepare_cached(
                "SELECT COUNT(*),
                        COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0),
                        COALESCE(SUM(latency_ms), 0),
                        COALESCE(SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END), 0)
                 FROM request_logs",
            )
            .ok()?;
        stmt.query_row([], |row| {
            stats.total_requests = row.get::<_, i64>(0)?.max(0) as u64;
            stats.total_input_tokens = row.get::<_, i64>(1)?.max(0) as u64;
            stats.total_output_tokens = row.get::<_, i64>(2)?.max(0) as u64;
            stats.total_latency_ms = row.get::<_, i64>(3)?.max(0) as u64;
            stats.total_errors = row.get::<_, i64>(4)?.max(0) as u64;
            Ok(())
        })
        .ok()?;
    }

    // 按模型聚合
    {
        let mut stmt = conn
            .prepare_cached(
                "SELECT model, COUNT(*),
                        COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0)
                 FROM request_logs GROUP BY model",
            )
            .ok()?;
        let rows = stmt
            .query_map([], |row| {
                let model: Option<String> = row.get(0)?;
                Ok((
                    model.unwrap_or_default(),
                    ModelStats {
                        requests: row.get::<_, i64>(1)?.max(0) as u64,
                        input_tokens: row.get::<_, i64>(2)?.max(0) as u64,
                        output_tokens: row.get::<_, i64>(3)?.max(0) as u64,
                    },
                ))
            })
            .ok()?;
        for row in rows.flatten() {
            stats.by_model.insert(row.0, row.1);
        }
    }

    // 最近 24 小时按小时聚合
    {
        let since = chrono::Utc::now().timestamp() - 86400;
        let mut stmt = conn
            .prepare_cached(
                "SELECT (timestamp / 3600) * 3600 AS hour_key, COUNT(*),
                        COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0)
                 FROM request_logs WHERE timestamp >= ?1 GROUP BY hour_key",
            )
            .ok()?;
        let rows = stmt
            .query_map(rusqlite::params![since], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    HourlyStats {
                        requests: row.get::<_, i64>(1)?.max(0) as u64,
                        input_tokens: row.get::<_, i64>(2)?.max(0) as u64,
                        output_tokens: row.get::<_, i64>(3)?.max(0) as u64,
                    },
                ))
            })
            .ok()?;
        for row in rows.flatten() {
            stats.by_hour.insert(row.0, row.1);
        }
    }

    stats.avg_latency_ms = stats
        .total_latency_ms
        .checked_div(stats.total_requests.max(1))
        .unwrap_or(0);

    Some(stats)
}

/// 基于内存日志的聚合（DB 不可用时的兜底路径）
fn stats_from_iter<'a>(logs: impl Iterator<Item = &'a RequestLog>) -> UsageStats {
    let mut stats = UsageStats::default();
    let now = chrono::Utc::now().timestamp();

    for log in logs {
        stats.total_requests += 1;
        stats.total_input_tokens += log.input_tokens;
        stats.total_output_tokens += log.output_tokens;
        stats.total_latency_ms += log.latency_ms;

        if log.status_code >= 400 {
            stats.total_errors += 1;
        }

        // 按模型聚合
        let entry = stats
            .by_model
            .entry(log.model.clone())
            .or_insert(ModelStats::default());
        entry.requests += 1;
        entry.input_tokens += log.input_tokens;
        entry.output_tokens += log.output_tokens;

        // 按时段聚合（最近24小时按小时）
        let secs_ago = now - log.timestamp;
        if secs_ago < 86400 && secs_ago >= 0 {
            let hour_key = (log.timestamp / 3600) * 3600;
            let h_entry = stats
                .by_hour
                .entry(hour_key)
                .or_insert(HourlyStats::default());
            h_entry.requests += 1;
            h_entry.input_tokens += log.input_tokens;
            h_entry.output_tokens += log.output_tokens;
        }
    }

    stats.avg_latency_ms = stats
        .total_latency_ms
        .checked_div(stats.total_requests.max(1))
        .unwrap_or(0);

    stats
}

// ── 启动时维护（同步上下文调用） ────────────────────────────────

/// 从 SQLite 加载最近的日志到内存（启动时调用，按时间正序填充 VecDeque）
pub fn load_recent_logs_sync(conn: &rusqlite::Connection) -> VecDeque<RequestLog> {
    let sql = format!(
        "SELECT {} FROM request_logs ORDER BY timestamp DESC, rowid DESC LIMIT ?1",
        LOG_COLUMNS
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return VecDeque::new(),
    };
    let rows = match stmt.query_map(rusqlite::params![MAX_LOGS_IN_MEMORY as i64], row_to_log) {
        Ok(r) => r,
        Err(_) => return VecDeque::new(),
    };
    // 查询结果为最新在前，反转为时间正序（VecDeque 尾部为最新）
    let mut logs: Vec<RequestLog> = rows.filter_map(|r| r.ok()).collect();
    logs.reverse();
    logs.into()
}

/// 清理过期与超量日志（启动时调用，防止数据库无限增长）
pub fn cleanup_old_logs_sync(conn: &rusqlite::Connection) {
    let cutoff = chrono::Utc::now().timestamp() - LOG_RETENTION_DAYS * 86400;
    let _ = conn.execute(
        "DELETE FROM request_logs WHERE timestamp < ?1",
        rusqlite::params![cutoff],
    );
    // 总量兜底：只保留最新 MAX_LOGS_IN_DB 条
    let _ = conn.execute(
        "DELETE FROM request_logs WHERE rowid NOT IN (
            SELECT rowid FROM request_logs ORDER BY timestamp DESC, rowid DESC LIMIT ?1
        )",
        rusqlite::params![MAX_LOGS_IN_DB],
    );
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UsageStats {
    pub total_requests: u64,
    pub total_errors: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_latency_ms: u64,
    pub avg_latency_ms: u64,
    pub by_model: std::collections::HashMap<String, ModelStats>,
    pub by_hour: std::collections::HashMap<i64, HourlyStats>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ModelStats {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct HourlyStats {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}
