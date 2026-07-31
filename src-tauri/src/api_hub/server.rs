use super::router::{route_by_model, RouteResult};
use super::transform::streaming::{StreamDirection, StreamState, transform_sse_line};
use super::types::{ApiProtocol, AppState};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::Method,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::TryStreamExt;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// 客户端请求所使用的格式（由命中的入口端点决定）
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClientFormat {
    OpenAIChat,
    OpenAIResponses,
    Anthropic,
}

/// 启动 API Hub HTTP 服务（绑定 localhost:3456）
pub async fn start_server(state: Arc<AppState>) {
    start_server_on(state, "127.0.0.1:3456").await;
}

/// 启动 API Hub HTTP 服务到指定地址（测试可绑定 `127.0.0.1:0`）
pub async fn start_server_on(state: Arc<AppState>, addr: &str) {
    let app = build_router(state.clone());

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => {
            if let Ok(local) = l.local_addr() {
                println!("[API Hub] Server started on http://{}", local);
            }
            state.running.store(true, Ordering::SeqCst);
            l
        }
        Err(e) => {
            eprintln!("[API Hub] Failed to bind {}: {}", addr, e);
            return;
        }
    };

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("[API Hub] Server error: {}", e);
    }
    state.running.store(false, Ordering::SeqCst);
}

/// 构建 Router（供集成测试 / 冒烟示例使用）
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/responses", post(responses_handler))
        .route("/v1/messages", post(anthropic_messages_handler))
        .route("/v1/models", get(list_models_handler))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MB
        .layer(cors)
        .with_state(state)
}

// ── Health ────────────────────────────────────────────────────

async fn health_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "server": "DevNexus API Hub",
        "port": 3456,
        "running": state.running.load(Ordering::SeqCst),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// ── Handlers ──────────────────────────────────────────────────

async fn chat_completions_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    handle_unified(state, body, ClientFormat::OpenAIChat).await
}

async fn responses_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    handle_unified(state, body, ClientFormat::OpenAIResponses).await
}

async fn anthropic_messages_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    handle_unified(state, body, ClientFormat::Anthropic).await
}

/// 统一的请求处理流程：
/// 客户端格式 → 内部 OpenAIChat → Provider 协议 → 转发 → Provider 响应 → 内部 → 客户端格式
async fn handle_unified(
    state: Arc<AppState>,
    body: serde_json::Value,
    client: ClientFormat,
) -> Response {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();

    if model.is_empty() {
        return error_response(400, "model field is required");
    }

    let route = match route_by_model(&state, &model).await {
        Some(r) => r,
        None => {
            return error_response(
                404,
                &format!("No provider found for model '{}'. Ensure the model is registered in a Provider's model list.", model),
            );
        }
    };

    let is_streaming = body
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    // 1. 客户端格式 → 内部 OpenAIChat 格式
    let internal_req = match client_request_to_internal(client, &body) {
        Ok(b) => b,
        Err(e) => return error_response(500, &e),
    };

    // 2. 内部格式 → Provider 协议格式
    let upstream_body = match internal_request_to_provider(&internal_req, &route) {
        Ok(b) => b,
        Err(e) => return error_response(500, &e),
    };

    let endpoint = route.provider.protocol.endpoint();

    if is_streaming {
        return handle_streaming(&state, &route, endpoint, upstream_body, client).await;
    }

    // 3. 转发到上游
    let (resp_body, _status) =
        match super::forwarder::forward_request(&state, &route.provider, endpoint, upstream_body)
            .await
        {
            Ok(r) => r,
            Err(e) => return error_response(502, &e),
        };

    // 4. Provider 响应 → 内部 OpenAIChat
    let internal_resp = match provider_response_to_internal(&resp_body, &route) {
        Ok(b) => b,
        Err(e) => return error_response(500, &e),
    };

    // 5. 内部 → 客户端格式
    match internal_response_to_client(&internal_resp, client, &model) {
        Ok(b) => Json(b).into_response(),
        Err(e) => error_response(500, &e),
    }
}

// ── List Models ───────────────────────────────────────────────

async fn list_models_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let providers = state.providers.read().await;
    let mut models: Vec<serde_json::Value> = Vec::new();

    for p in providers.iter() {
        if !p.enabled {
            continue;
        }
        for m in &p.models {
            models.push(serde_json::json!({
                "id": m,
                "provider": p.name,
                "protocol": p.protocol.as_str(),
                "owned_by": p.name,
            }));
        }
    }

    Json(serde_json::json!({
        "object": "list",
        "data": models
    }))
}

// ── Format conversion ─────────────────────────────────────────

/// 客户端请求 → 内部 OpenAIChat 格式
fn client_request_to_internal(
    client: ClientFormat,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match client {
        ClientFormat::OpenAIChat => Ok(body.clone()),
        ClientFormat::OpenAIResponses => Ok(super::transform::responses::responses_to_chat(body)),
        ClientFormat::Anthropic => {
            let req: super::types::AnthropicRequest = serde_json::from_value(body.clone())
                .map_err(|e| format!("Invalid Anthropic request: {}", e))?;
            let oai = super::transform::anthropic::anthropic_to_openai_req(&req);
            serde_json::to_value(oai).map_err(|e| format!("Serialization error: {}", e))
        }
    }
}

/// 内部 OpenAIChat 请求 → Provider 协议格式
fn internal_request_to_provider(
    internal: &serde_json::Value,
    route: &RouteResult,
) -> Result<serde_json::Value, String> {
    match route.provider.protocol {
        ApiProtocol::OpenAIChat => Ok(internal.clone()),
        ApiProtocol::OpenAIResponses => {
            Ok(super::transform::responses::chat_request_to_responses(internal))
        }
        ApiProtocol::Anthropic => {
            let oai: super::types::OpenAIChatRequest = serde_json::from_value(internal.clone())
                .map_err(|e| format!("Invalid OpenAI request: {}", e))?;
            let anth = super::transform::anthropic::openai_to_anthropic(&oai);
            serde_json::to_value(anth).map_err(|e| format!("Serialization error: {}", e))
        }
    }
}

/// Provider 响应 → 内部 OpenAIChat 格式
fn provider_response_to_internal(
    resp: &serde_json::Value,
    route: &RouteResult,
) -> Result<serde_json::Value, String> {
    match route.provider.protocol {
        ApiProtocol::OpenAIChat => Ok(resp.clone()),
        ApiProtocol::OpenAIResponses => Ok(
            super::transform::responses::responses_to_chat_response(resp, &route.model),
        ),
        ApiProtocol::Anthropic => {
            let anth: super::types::AnthropicResponse = serde_json::from_value(resp.clone())
                .map_err(|e| format!("Invalid Anthropic response: {}", e))?;
            let oai =
                super::transform::anthropic::anthropic_to_openai(&anth.id, &route.model, &anth);
            serde_json::to_value(oai).map_err(|e| format!("Serialization error: {}", e))
        }
    }
}

/// 内部 OpenAIChat 响应 → 客户端格式
fn internal_response_to_client(
    internal: &serde_json::Value,
    client: ClientFormat,
    model: &str,
) -> Result<serde_json::Value, String> {
    match client {
        ClientFormat::OpenAIChat => Ok(internal.clone()),
        ClientFormat::OpenAIResponses => Ok(super::transform::responses::chat_to_responses(
            internal, model,
        )),
        ClientFormat::Anthropic => Ok(super::transform::anthropic::openai_response_to_anthropic(
            internal, model,
        )),
    }
}

// ── Streaming ────────────────────────────────────────────────

/// 处理流式请求：判断是否需要跨协议转换
async fn handle_streaming(
    state: &AppState,
    route: &RouteResult,
    endpoint: &str,
    upstream_body: serde_json::Value,
    client: ClientFormat,
) -> Response {
    let (resp, log_id) =
        match super::forwarder::forward_streaming(state, &route.provider, endpoint, upstream_body)
            .await
        {
            Ok(r) => r,
            Err(e) => return error_response(502, &e),
        };

    let status = axum::http::StatusCode::from_u16(resp.status().as_u16())
        .unwrap_or(axum::http::StatusCode::OK);
    let headers = resp.headers().clone();

    // Determine if we need format conversion
    let direction = determine_stream_direction(client, route.provider.protocol);

    // 在上游字节流上包一层 usage 捕获：流结束后回填 token 用量到日志
    let byte_stream = capture_usage_stream(resp.bytes_stream(), state.clone(), log_id);

    if let Some(dir) = direction {
        // Cross-protocol: transform each SSE line
        let transformed = transform_byte_stream(byte_stream, dir);
        let body = axum::body::Body::from_stream(transformed);

        let content_type = match client {
            ClientFormat::Anthropic => "text/event-stream",
            _ => "text/event-stream",
        };

        Response::builder()
            .status(status)
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .header("Content-Type", content_type)
            .body(body)
            .unwrap_or_else(|_| error_response(500, "Stream build error"))
    } else {
        // Same protocol: passthrough bytes directly (zero overhead)
        let stream = byte_stream.map_err(|e| {
            std::io::Error::other(format!("Stream error: {}", e))
        });
        let body = axum::body::Body::from_stream(stream);

        let mut response_builder = Response::builder()
            .status(status)
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive");

        if let Some(content_type) = headers.get("content-type") {
            response_builder = response_builder.header("Content-Type", content_type);
        }

        response_builder
            .body(body)
            .unwrap_or_else(|_| error_response(500, "Stream build error"))
    }
}

/// Determine if cross-protocol stream conversion is needed.
/// Returns None if same protocol (passthrough), Some(direction) if conversion needed.
fn determine_stream_direction(
    client: ClientFormat,
    provider_protocol: ApiProtocol,
) -> Option<StreamDirection> {
    let client_protocol = match client {
        ClientFormat::OpenAIChat => ApiProtocol::OpenAIChat,
        ClientFormat::OpenAIResponses => ApiProtocol::OpenAIResponses,
        ClientFormat::Anthropic => ApiProtocol::Anthropic,
    };

    if client_protocol == provider_protocol {
        return None; // Same protocol, passthrough
    }

    // Provider produces in its protocol format; we need to convert to client format.
    // The stream from provider is in provider_protocol format.
    // We need to convert it to client_protocol format.
    // Since our internal format is OpenAI Chat, conversion paths:
    match (provider_protocol, client_protocol) {
        // Provider is OpenAI Chat, Client wants Anthropic
        (ApiProtocol::OpenAIChat, ApiProtocol::Anthropic) => {
            Some(StreamDirection::OpenAIChatToAnthropic)
        }
        // Provider is OpenAI Chat, Client wants Responses
        (ApiProtocol::OpenAIChat, ApiProtocol::OpenAIResponses) => {
            Some(StreamDirection::OpenAIChatToResponses)
        }
        // Provider is Anthropic, Client wants OpenAI Chat
        (ApiProtocol::Anthropic, ApiProtocol::OpenAIChat) => {
            Some(StreamDirection::AnthropicToOpenAIChat)
        }
        // Provider is Anthropic, Client wants Responses
        // Two-step: Anthropic → OpenAI Chat → Responses
        // For simplicity, use Anthropic → OpenAI Chat (client gets OpenAI Chat which is close enough)
        (ApiProtocol::Anthropic, ApiProtocol::OpenAIResponses) => {
            // Convert to OpenAI Chat first (closest available)
            Some(StreamDirection::AnthropicToOpenAIChat)
        }
        // Provider is Responses, Client wants OpenAI Chat
        (ApiProtocol::OpenAIResponses, ApiProtocol::OpenAIChat) => {
            Some(StreamDirection::ResponsesToOpenAIChat)
        }
        // Provider is Responses, Client wants Anthropic
        (ApiProtocol::OpenAIResponses, ApiProtocol::Anthropic) => {
            // Convert to OpenAI Chat first
            Some(StreamDirection::ResponsesToOpenAIChat)
        }
        _ => None,
    }
}

/// 包装上游字节流：数据原样透传，同时扫描 SSE 行提取 usage token 用量，
/// 流结束后异步回填到请求日志（解决流式请求 token 统计为 0 的问题）。
fn capture_usage_stream(
    byte_stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
    state: AppState,
    log_id: String,
) -> impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static {
    async_stream::stream! {
        use futures_util::StreamExt;
        // 扫描缓冲上限：防御异常上游把单行撑得过大占用内存
        const MAX_SCAN_BUF: usize = 256 * 1024;
        let mut scan_buf = String::new();
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;

        futures_util::pin_mut!(byte_stream);

        while let Some(chunk_result) = byte_stream.next().await {
            if let Ok(ref chunk) = chunk_result {
                if scan_buf.len() < MAX_SCAN_BUF {
                    scan_buf.push_str(&String::from_utf8_lossy(chunk));
                    while let Some(pos) = scan_buf.find('\n') {
                        let line = scan_buf[..pos].trim_end_matches('\r').to_string();
                        scan_buf.drain(..=pos);
                        extract_usage_from_sse_line(&line, &mut input_tokens, &mut output_tokens);
                    }
                }
            }
            yield chunk_result;
        }

        // 处理残余缓冲，然后回填 token 用量
        let tail = scan_buf.trim().to_string();
        extract_usage_from_sse_line(&tail, &mut input_tokens, &mut output_tokens);
        if input_tokens > 0 || output_tokens > 0 {
            tokio::spawn(async move {
                super::usage::update_log_tokens(&state, &log_id, input_tokens, output_tokens).await;
            });
        }
    }
}

/// 从单行 SSE data 中提取 usage 字段（兼容 OpenAI Chat / Responses / Anthropic 三种格式）
fn extract_usage_from_sse_line(line: &str, input: &mut u64, output: &mut u64) {
    let data = match line.strip_prefix("data:") {
        Some(d) => d.trim(),
        None => return,
    };
    // 快速路径：不含 usage 字样的行直接跳过，避免逐 chunk 反序列化
    if data.is_empty() || data == "[DONE]" || !data.contains("\"usage\"") {
        return;
    }
    let json: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    // usage 可能位于顶层（OpenAI Chat / Anthropic message_delta）、
    // message 下（Anthropic message_start）或 response 下（Responses response.completed）
    let usage = json
        .get("usage")
        .or_else(|| json.get("message").and_then(|m| m.get("usage")))
        .or_else(|| json.get("response").and_then(|r| r.get("usage")));
    let Some(usage) = usage else { return };

    let in_val = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(|v| v.as_u64());
    let out_val = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(|v| v.as_u64());
    if let Some(v) = in_val {
        *input = (*input).max(v);
    }
    if let Some(v) = out_val {
        *output = (*output).max(v);
    }
}

/// Transform a byte stream using SSE line-by-line conversion.
fn transform_byte_stream(
    byte_stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
    direction: StreamDirection,
) -> impl futures_util::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send + 'static {
    async_stream::stream! {
        use futures_util::StreamExt;
        let mut state = StreamState::default();
        let mut buffer = String::new();

        futures_util::pin_mut!(byte_stream);

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    yield Err(std::io::Error::other(format!("Upstream error: {}", e)));
                    return;
                }
            };

            // Append chunk bytes to buffer
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete lines
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    // Empty line = SSE event separator, skip
                    continue;
                }

                // Skip event: lines (we handle them via data: parsing)
                if line.starts_with("event:") {
                    continue;
                }

                let output_lines = transform_sse_line(direction, &line, &mut state);
                for out_line in output_lines {
                    let formatted = format!("{}\n", out_line);
                    yield Ok(bytes::Bytes::from(formatted));
                }
            }
        }

        // Process any remaining buffer
        if !buffer.trim().is_empty() {
            let output_lines = transform_sse_line(direction, buffer.trim(), &mut state);
            for out_line in output_lines {
                let formatted = format!("{}\n", out_line);
                yield Ok(bytes::Bytes::from(formatted));
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

fn error_response(status: u16, message: &str) -> Response {
    let body = serde_json::json!({
        "error": {
            "message": message,
            "type": "api_hub_error",
            "code": status
        }
    });
    (
        axum::http::StatusCode::from_u16(status)
            .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
        Json(body),
    )
        .into_response()
}
