use super::router::build_upstream_url;
use super::types::{ApiProtocol, AppState, Provider, RequestLog};
use std::time::Instant;

/// 转发请求到上游 Provider 并获取响应（使用共享 HTTP Client）
pub async fn forward_request(
    state: &AppState,
    provider: &Provider,
    endpoint: &str,
    body: serde_json::Value,
) -> Result<(serde_json::Value, u16), String> {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();
    let url = build_upstream_url(provider, endpoint);
    let start = Instant::now();

    // 构建 HTTP 请求（使用全局共享 Client）
    let mut req_builder = state.http_client.post(&url).json(&body);

    // 添加认证头
    req_builder = apply_auth_headers(req_builder, provider);

    // 发送请求
    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            log_error(state, provider, &model, elapsed, 0, Some(&e.to_string())).await;
            return Err(format!("Request failed: {}", e));
        }
    };

    let status = resp.status().as_u16();
    let elapsed = start.elapsed().as_millis() as u64;

    if status >= 400 {
        let error_body = resp.text().await.unwrap_or_default();
        log_error(state, provider, &model, elapsed, status, Some(&error_body)).await;
        return Err(format!("Upstream error ({}): {}", status, error_body));
    }

    let json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(e) => {
            log_error(
                state,
                provider,
                &model,
                elapsed,
                status,
                Some(&e.to_string()),
            )
            .await;
            return Err(format!("Failed to parse response: {}", e));
        }
    };

    // 记录成功日志
    log_success(state, provider, &model, &json, elapsed, status).await;

    Ok((json, status))
}

/// 对流式响应进行转发，返回 (reqwest::Response, 日志 ID) 供上层处理与 token 回填
pub async fn forward_streaming(
    state: &AppState,
    provider: &Provider,
    endpoint: &str,
    mut body: serde_json::Value,
) -> Result<(reqwest::Response, String), String> {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();

    // OpenAI Chat 协议：请求上游在流尾部附带 usage，便于 token 统计
    if provider.protocol == ApiProtocol::OpenAIChat {
        if let Some(obj) = body.as_object_mut() {
            if !obj.contains_key("stream_options") {
                obj.insert(
                    "stream_options".to_string(),
                    serde_json::json!({ "include_usage": true }),
                );
            }
        }
    }

    let url = build_upstream_url(provider, endpoint);
    let start = Instant::now();

    let mut req_builder = state.http_client.post(&url).json(&body);
    req_builder = apply_auth_headers(req_builder, provider);

    let resp = req_builder.send().await.map_err(|e| {
        let elapsed = start.elapsed().as_millis() as u64;
        // Fire-and-forget error log (async context)
        let state_clone = state.clone();
        let provider_clone = provider.clone();
        let model_clone = model.clone();
        let err_str = e.to_string();
        tokio::spawn(async move {
            log_error(
                &state_clone,
                &provider_clone,
                &model_clone,
                elapsed,
                0,
                Some(&err_str),
            )
            .await;
        });
        format!("Stream request failed: {}", e)
    })?;

    let status = resp.status().as_u16();
    if status >= 400 {
        let elapsed = start.elapsed().as_millis() as u64;
        log_error(state, provider, &model, elapsed, status, None).await;
        return Err(format!("Upstream error: {}", resp.status()));
    }

    // Log streaming start; token 用量在流结束后由上层通过 update_log_tokens 回填
    let elapsed = start.elapsed().as_millis() as u64;
    let log_id = uuid::Uuid::new_v4().to_string();
    let log = RequestLog {
        id: log_id.clone(),
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        model: model.clone(),
        request_model: model,
        input_tokens: 0,
        output_tokens: 0,
        latency_ms: elapsed,
        status_code: status,
        error_message: None,
        timestamp: chrono::Utc::now().timestamp(),
        is_streaming: true,
    };
    super::usage::log_request(state, log).await;

    Ok((resp, log_id))
}

// ── Auth Headers ─────────────────────────────────────────────

fn apply_auth_headers(
    req_builder: reqwest::RequestBuilder,
    provider: &Provider,
) -> reqwest::RequestBuilder {
    match provider.protocol {
        ApiProtocol::Anthropic => req_builder
            .header("x-api-key", &provider.api_key)
            .header("anthropic-version", "2023-06-01"),
        ApiProtocol::OpenAIChat | ApiProtocol::OpenAIResponses => {
            if !provider.api_key.is_empty() {
                req_builder.header("Authorization", format!("Bearer {}", provider.api_key))
            } else {
                req_builder
            }
        }
    }
}

// ── Logging ──────────────────────────────────────────────────

/// 记录成功请求
async fn log_success(
    state: &AppState,
    provider: &Provider,
    model: &str,
    resp_body: &serde_json::Value,
    latency_ms: u64,
    status_code: u16,
) {
    let (input_tokens, output_tokens) = extract_tokens(resp_body, &provider.protocol);

    let log = RequestLog {
        id: uuid::Uuid::new_v4().to_string(),
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        model: model.to_string(),
        request_model: model.to_string(),
        input_tokens,
        output_tokens,
        latency_ms,
        status_code,
        error_message: None,
        timestamp: chrono::Utc::now().timestamp(),
        is_streaming: false,
    };

    super::usage::log_request(state, log).await;
}

/// 记录错误请求
async fn log_error(
    state: &AppState,
    provider: &Provider,
    model: &str,
    latency_ms: u64,
    status_code: u16,
    error: Option<&str>,
) {
    let log = RequestLog {
        id: uuid::Uuid::new_v4().to_string(),
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        model: model.to_string(),
        request_model: model.to_string(),
        input_tokens: 0,
        output_tokens: 0,
        latency_ms,
        status_code,
        error_message: error.map(|s| s.to_string()),
        timestamp: chrono::Utc::now().timestamp(),
        is_streaming: false,
    };

    super::usage::log_request(state, log).await;
}

/// 从响应中提取 Token 计数
fn extract_tokens(resp: &serde_json::Value, protocol: &ApiProtocol) -> (u64, u64) {
    match protocol.token_scheme() {
        super::types::TokenScheme::PromptCompletion => {
            let usage = resp.get("usage");
            let input = usage
                .and_then(|u| u.get("prompt_tokens"))
                .and_then(|t| t.as_u64())
                .unwrap_or(0);
            let output = usage
                .and_then(|u| u.get("completion_tokens"))
                .and_then(|t| t.as_u64())
                .unwrap_or(0);
            (input, output)
        }
        super::types::TokenScheme::InputOutput => {
            let usage = resp.get("usage");
            let input = usage
                .and_then(|u| u.get("input_tokens"))
                .and_then(|t| t.as_u64())
                .unwrap_or(0);
            let output = usage
                .and_then(|u| u.get("output_tokens"))
                .and_then(|t| t.as_u64())
                .unwrap_or(0);
            (input, output)
        }
    }
}
