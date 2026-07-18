use super::router::build_upstream_url;
use super::types::{ApiProtocol, AppState, Provider, RequestLog};
use std::time::Instant;

/// 转发请求到上游 Provider 并获取响应
pub async fn forward_request(
    state: &AppState,
    provider: &Provider,
    endpoint: &str,
    body: serde_json::Value,
) -> Result<(serde_json::Value, u16), String> {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("");
    let url = build_upstream_url(provider, endpoint, model);
    let start = Instant::now();

    // Gemini 请求体不含 model 字段（模型在 URL 路径中）
    let mut body = body;
    if matches!(provider.protocol, ApiProtocol::Gemini) {
        if let Some(obj) = body.as_object_mut() {
            obj.remove("model");
        }
    }

    // 构建 HTTP 请求
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Gemini 特殊处理：API key 作为查询参数
    let request_url = match provider.protocol {
        ApiProtocol::Gemini if !provider.api_key.is_empty() => {
            format!("{}?key={}", url, provider.api_key)
        }
        _ => url.clone(),
    };

    let mut req_builder = client.post(&request_url).json(&body);

    // 添加认证头
    match provider.protocol {
        ApiProtocol::Anthropic => {
            req_builder = req_builder
                .header("x-api-key", &provider.api_key)
                .header("anthropic-version", "2023-06-01");
        }
        ApiProtocol::OpenAIChat | ApiProtocol::OpenAIResponses => {
            if !provider.api_key.is_empty() {
                req_builder = req_builder
                    .header("Authorization", format!("Bearer {}", provider.api_key));
            }
        }
        ApiProtocol::Gemini | ApiProtocol::Ollama => {
            // Gemini key already in URL; Ollama needs no auth
        }
    }

    // 发送请求
    let resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            log_error(state, provider, &body, elapsed, 0, Some(&e.to_string()));
            return Err(format!("Request failed: {}", e));
        }
    };

    let status = resp.status().as_u16();
    let elapsed = start.elapsed().as_millis() as u64;

    if status >= 400 {
        let error_body = resp.text().await.unwrap_or_default();
        log_error(state, provider, &body, elapsed, status, Some(&error_body));
        return Err(format!("Upstream error ({}): {}", status, error_body));
    }

    let json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(e) => {
            log_error(state, provider, &body, elapsed, status, Some(&e.to_string()));
            return Err(format!("Failed to parse response: {}", e));
        }
    };

    // 记录成功日志
    log_success(state, provider, &body, &json, elapsed, status);

    Ok((json, status))
}

/// 记录成功请求
fn log_success(
    state: &AppState,
    provider: &Provider,
    req_body: &serde_json::Value,
    resp_body: &serde_json::Value,
    latency_ms: u64,
    status_code: u16,
) {
    let model = req_body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");

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
        is_streaming: req_body.get("stream").and_then(|s| s.as_bool()).unwrap_or(false),
    };

    super::usage::log_request(state, log);
}

/// 记录错误请求
fn log_error(
    state: &AppState,
    provider: &Provider,
    req_body: &serde_json::Value,
    latency_ms: u64,
    status_code: u16,
    error: Option<&str>,
) {
    let model = req_body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");

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

    super::usage::log_request(state, log);
}

/// 从响应中提取 Token 计数
fn extract_tokens(resp: &serde_json::Value, protocol: &ApiProtocol) -> (u32, u32) {
    match protocol.token_scheme() {
        super::types::TokenScheme::PromptCompletion => {
            let usage = resp.get("usage");
            let input = usage
                .and_then(|u| u.get("prompt_tokens"))
                .and_then(|t| t.as_u64())
                .unwrap_or(0) as u32;
            let output = usage
                .and_then(|u| u.get("completion_tokens"))
                .and_then(|t| t.as_u64())
                .unwrap_or(0) as u32;
            (input, output)
        }
        super::types::TokenScheme::InputOutput => {
            let usage = resp.get("usage");
            let input = usage
                .and_then(|u| u.get("input_tokens"))
                .and_then(|t| t.as_u64())
                .unwrap_or(0) as u32;
            let output = usage
                .and_then(|u| u.get("output_tokens"))
                .and_then(|t| t.as_u64())
                .unwrap_or(0) as u32;
            (input, output)
        }
        super::types::TokenScheme::None => (0, 0),
    }
}

/// 对流式响应进行 SSE 转发
pub async fn forward_streaming(
    state: &AppState,
    provider: &Provider,
    endpoint: &str,
    body: serde_json::Value,
) -> Result<reqwest::Response, String> {
    // Gemini body 无 model 字段，优先用调用方 endpoint 模板中的模型
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("");
    let url = build_upstream_url(provider, endpoint, model);
    let start = Instant::now();

    let mut body = body;
    if matches!(provider.protocol, ApiProtocol::Gemini) {
        if let Some(obj) = body.as_object_mut() {
            obj.remove("model");
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let request_url = match provider.protocol {
        ApiProtocol::Gemini if !provider.api_key.is_empty() => {
            format!("{}?key={}", url, provider.api_key)
        }
        _ => url,
    };

    let mut req_builder = client.post(&request_url).json(&body);

    match provider.protocol {
        ApiProtocol::Anthropic => {
            req_builder = req_builder
                .header("x-api-key", &provider.api_key)
                .header("anthropic-version", "2023-06-01");
        }
        ApiProtocol::OpenAIChat | ApiProtocol::OpenAIResponses => {
            if !provider.api_key.is_empty() {
                req_builder = req_builder
                    .header("Authorization", format!("Bearer {}", provider.api_key));
            }
        }
        ApiProtocol::Gemini | ApiProtocol::Ollama => {}
    }

    let resp = req_builder.send().await.map_err(|e| {
        let elapsed = start.elapsed().as_millis() as u64;
        log_error(state, provider, &body, elapsed, 0, Some(&e.to_string()));
        format!("Stream request failed: {}", e)
    })?;

    let elapsed = start.elapsed().as_millis() as u64;
    if resp.status().as_u16() >= 400 {
        log_error(state, provider, &body, elapsed, resp.status().as_u16(), None);
        return Err(format!("Upstream error: {}", resp.status()));
    }

    Ok(resp)
}
