use super::types::{AppState, RequestLog};

/// 记录一次请求到日志
pub fn log_request(state: &AppState, log: RequestLog) {
    // 内存中保留最近 1000 条
    if let Ok(mut logs) = state.request_logs.write() {
        logs.push(log.clone());
        if logs.len() > 1000 {
            logs.remove(0);
        }
    }

    // 持久化到 SQLite
    if let Ok(db) = state.db.lock() {
        if let Some(ref conn) = *db {
            let _ = conn.execute(
                "INSERT INTO request_logs (id, provider_id, provider_name, model, request_model,
                 input_tokens, output_tokens, latency_ms, status_code, error_message, timestamp, is_streaming)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                rusqlite::params![
                    log.id,
                    log.provider_id,
                    log.provider_name,
                    log.model,
                    log.request_model,
                    log.input_tokens,
                    log.output_tokens,
                    log.latency_ms,
                    log.status_code,
                    log.error_message,
                    log.timestamp,
                    log.is_streaming as i32,
                ],
            );
        }
    }
}

/// 获取请求日志列表（支持分页）
pub fn get_logs(state: &AppState, limit: usize, offset: usize) -> Vec<RequestLog> {
    if let Ok(logs) = state.request_logs.read() {
        let mut all: Vec<RequestLog> = logs.clone();
        all.reverse(); // 最新的在前
        all.into_iter().skip(offset).take(limit).collect()
    } else {
        Vec::new()
    }
}

/// 获取用量统计数据
pub fn get_usage_stats(state: &AppState) -> UsageStats {
    let logs = if let Ok(l) = state.request_logs.read() {
        l.clone()
    } else {
        return UsageStats::default();
    };

    let mut stats = UsageStats::default();
    let now = chrono::Utc::now().timestamp();

    for log in &logs {
        stats.total_requests += 1;
        stats.total_input_tokens += log.input_tokens;
        stats.total_output_tokens += log.output_tokens;
        stats.total_latency_ms += log.latency_ms;

        if log.status_code >= 400 {
            stats.total_errors += 1;
        }

        // 按模型聚合
        let entry = stats.by_model.entry(log.model.clone()).or_insert(ModelStats::default());
        entry.requests += 1;
        entry.input_tokens += log.input_tokens;
        entry.output_tokens += log.output_tokens;

        // 按时段聚合（最近24小时按小时）
        let secs_ago = now - log.timestamp;
        if secs_ago < 86400 {
            let hour_key = (log.timestamp / 3600) * 3600;
            let h_entry = stats.by_hour.entry(hour_key).or_insert(HourlyStats::default());
            h_entry.requests += 1;
            h_entry.input_tokens += log.input_tokens;
            h_entry.output_tokens += log.output_tokens;
        }
    }

    stats.avg_latency_ms = if stats.total_requests > 0 {
        stats.total_latency_ms / stats.total_requests as u64
    } else {
        0
    };

    stats
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UsageStats {
    pub total_requests: u64,
    pub total_errors: u64,
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_latency_ms: u64,
    pub avg_latency_ms: u64,
    pub by_model: std::collections::HashMap<String, ModelStats>,
    pub by_hour: std::collections::HashMap<i64, HourlyStats>, // timestamp(秒) → 统计
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ModelStats {
    pub requests: u64,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct HourlyStats {
    pub requests: u64,
    pub input_tokens: u32,
    pub output_tokens: u32,
}
