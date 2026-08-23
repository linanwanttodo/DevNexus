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
            err.push('…');
        }
    }

    // 内存中保留最近 MAX_LOGS_IN_MEMORY 条（批量淘汰，避免逐条 pop 抖动）
    {
        let mut logs = state.request_logs.write().await;
        logs.push_back(log.clone());
        if logs.len() > MAX_LOGS_IN_MEMORY {
            let overflow = logs.len() - MAX_LOGS_IN_MEMORY;
            logs.drain(0..overflow);
        }
    }

    // 异步持久化到 SQLite（await 确保 INSERT 提交后再返回，避免流式结束后
    // update_log_tokens 的 UPDATE 先于 INSERT 执行导致 token 统计丢失）
    let db = state.db.clone();
    let res = tokio::task::spawn_blocking(move || {
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
    if res.is_err() {
        eprintln!("[API Hub] log_request spawn_blocking join failed");
    }
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
    let res = tokio::task::spawn_blocking(move || {
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
    if res.is_err() {
        eprintln!("[API Hub] update_log_tokens spawn_blocking join failed");
    }
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
    if db_result.is_none() {
        eprintln!("[API Hub] get_logs: DB unavailable or query failed, falling back to memory");
    }

    if let Some(logs) = db_result {
        return logs;
    }

    // 回退：内存日志（从尾部最新开始）
    let logs = state.request_logs.read().await;
    logs.iter()
        .rev()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect()
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
    if db_result.is_none() {
        eprintln!(
            "[API Hub] get_usage_stats: DB unavailable or query failed, falling back to memory"
        );
    }

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
        .query_map(rusqlite::params![limit as i64, offset as i64], row_to_log)
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

    // 最近 365 天按天聚合（GitHub 风格贡献热力图数据源）
    {
        let since = chrono::Utc::now().timestamp() - 365 * 86400;
        let mut stmt = conn
            .prepare_cached(
                "SELECT (timestamp / 86400) * 86400 AS day_key, COUNT(*),
                        COALESCE(SUM(input_tokens), 0),
                        COALESCE(SUM(output_tokens), 0)
                 FROM request_logs WHERE timestamp >= ?1 GROUP BY day_key",
            )
            .ok()?;
        let rows = stmt
            .query_map(rusqlite::params![since], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    DailyStats {
                        requests: row.get::<_, i64>(1)?.max(0) as u64,
                        input_tokens: row.get::<_, i64>(2)?.max(0) as u64,
                        output_tokens: row.get::<_, i64>(3)?.max(0) as u64,
                    },
                ))
            })
            .ok()?;
        for row in rows.flatten() {
            stats.by_day.insert(row.0, row.1);
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
        if (0..86400).contains(&secs_ago) {
            let hour_key = (log.timestamp / 3600) * 3600;
            let h_entry = stats
                .by_hour
                .entry(hour_key)
                .or_insert(HourlyStats::default());
            h_entry.requests += 1;
            h_entry.input_tokens += log.input_tokens;
            h_entry.output_tokens += log.output_tokens;
        }

        // 按天聚合（最近 365 天 → GitHub 风格热力图）
        if secs_ago < 365 * 86400 {
            let day_key = (log.timestamp / 86400) * 86400;
            let d_entry = stats.by_day.entry(day_key).or_insert(DailyStats::default());
            d_entry.requests += 1;
            d_entry.input_tokens += log.input_tokens;
            d_entry.output_tokens += log.output_tokens;
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
    pub by_day: std::collections::HashMap<i64, DailyStats>,
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

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DailyStats {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_hub::types::RequestLog;

    fn log(
        id: &str,
        model: &str,
        input: u64,
        output: u64,
        latency: u64,
        status: u16,
        timestamp: i64,
    ) -> RequestLog {
        RequestLog {
            id: id.to_string(),
            provider_id: "p1".to_string(),
            provider_name: "Test".to_string(),
            model: model.to_string(),
            request_model: model.to_string(),
            input_tokens: input,
            output_tokens: output,
            latency_ms: latency,
            status_code: status,
            error_message: None,
            timestamp,
            is_streaming: false,
        }
    }

    #[test]
    fn test_stats_aggregation_empty() {
        let stats = stats_from_iter(std::iter::empty());
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.total_errors, 0);
        assert!(stats.by_model.is_empty());
        assert_eq!(stats.avg_latency_ms, 0);
    }

    #[test]
    fn test_stats_aggregation_counts_and_errors() {
        let now = chrono::Utc::now().timestamp();
        let logs = [
            log("1", "gpt-4", 10, 20, 100, 200, now),
            log("2", "gpt-4", 30, 40, 200, 500, now), // 错误
            log("3", "claude", 5, 5, 50, 200, now),
        ];
        let stats = stats_from_iter(logs.iter());
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.total_errors, 1);
        assert_eq!(stats.total_input_tokens, 45);
        assert_eq!(stats.total_output_tokens, 65);
        assert_eq!(stats.total_latency_ms, 350);
        assert_eq!(stats.avg_latency_ms, 116); // 350/3
        assert_eq!(stats.by_model["gpt-4"].requests, 2);
        assert_eq!(stats.by_model["claude"].requests, 1);
        // 最近 24h 内 → by_hour / by_day 有记录
        assert!(!stats.by_hour.is_empty());
        assert!(!stats.by_day.is_empty());
    }

    #[test]
    fn test_stats_hourly_buckets_only_recent() {
        let now = chrono::Utc::now().timestamp();
        let old = now - 2 * 86400; // 2 天前（超出小时桶，但在天桶内）
        let logs = [log("1", "m", 1, 1, 10, 200, old)];
        let stats = stats_from_iter(logs.iter());
        assert!(stats.by_hour.is_empty(), "旧日志不应进入小时桶");
        assert_eq!(stats.by_day.len(), 1, "旧日志应进入天桶");
    }

    #[test]
    fn test_stats_buckets_rollup() {
        // 锚点取"一小时前"：保证日志时间戳恒在过去（secs_ago > 0 才会进小时桶），
        // 且两条日志落在同一小时内——避免整点后几十秒内运行时第二条日志
        // 时间戳晚于 now 被过滤（时序竞态导致 flaky）。
        let now = chrono::Utc::now().timestamp();
        let anchor = now - 3600; // 一小时前
        let hour_start = (anchor / 3600) * 3600;
        let logs = [
            log("1", "m", 5, 5, 10, 200, hour_start + 60),
            log("2", "m", 5, 5, 10, 200, hour_start + 120),
        ];
        let stats = stats_from_iter(logs.iter());
        let h = &stats.by_hour[&hour_start];
        assert_eq!(h.requests, 2);
        assert_eq!(h.input_tokens, 10);
        assert_eq!(h.output_tokens, 10);
    }

    #[test]
    fn test_truncate_long_error_message() {
        // 模拟 log_request 的错误截断逻辑：超长错误信息应被截断到 MAX_ERROR_LEN
        let mut err = Some("e".repeat(2000));
        if let Some(ref mut e) = err {
            if e.len() > MAX_ERROR_LEN {
                let mut cut = MAX_ERROR_LEN;
                while !e.is_char_boundary(cut) {
                    cut -= 1;
                }
                e.truncate(cut);
                e.push('…');
            }
        }
        let err = err.unwrap();
        // 截断到 MAX_ERROR_LEN 字节 + 省略号（… 为 3 字节 UTF-8）
        assert_eq!(err.len(), MAX_ERROR_LEN + 3);
        assert!(err.ends_with('…'));
        assert!(err.starts_with("eeee"));
    }

    #[test]
    fn test_sqlite_log_roundtrip_and_cleanup() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::api_hub::provider::init_db_sync(&conn).expect("init db");

        // 插入两条日志
        let now = chrono::Utc::now().timestamp();
        for (i, status) in [(1i64, 200i64), (2, 500)] {
            conn.execute(
                "INSERT INTO request_logs (id, provider_id, provider_name, model, request_model,
                 input_tokens, output_tokens, latency_ms, status_code, error_message, timestamp, is_streaming)
                 VALUES (?1, 'p1', 'Test', 'gpt-4', 'gpt-4', 10, 20, 100, ?2, NULL, ?3, 0)",
                rusqlite::params![format!("log-{}", i), status, now],
            )
            .unwrap();
        }

        // 加载：按时间正序，两条都在
        let logs = load_recent_logs_sync(&conn);
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].id, "log-1");
        assert_eq!(logs[1].id, "log-2");

        // 清理不删新日志
        cleanup_old_logs_sync(&conn);
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM request_logs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 2);

        // 清理旧日志（30 天前）应被删除
        let ancient = now - 60 * 86400;
        conn.execute(
            "INSERT INTO request_logs (id, provider_id, provider_name, model, request_model,
             input_tokens, output_tokens, latency_ms, status_code, error_message, timestamp, is_streaming)
             VALUES ('log-old', 'p1', 'Test', 'gpt-4', 'gpt-4', 0, 0, 0, 200, NULL, ?1, 0)",
            rusqlite::params![ancient],
        )
        .unwrap();
        cleanup_old_logs_sync(&conn);
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM request_logs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 2);
    }
}
