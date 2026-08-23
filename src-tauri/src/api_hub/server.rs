use super::router::{route_by_model, RouteResult};
use super::transform::streaming::{transform_sse_line, StreamDirection, StreamState};
use super::types::{ApiProtocol, AppState};
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderName, HeaderValue, Method},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::TryStreamExt;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

/// 客户端请求所使用的格式（由命中的入口端点决定）
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClientFormat {
    OpenAIChat,
    OpenAIResponses,
    Anthropic,
    Gemini,
    Ollama,
}

/// 启动 API Hub HTTP 服务（绑定 localhost:3456）
pub async fn start_server(state: Arc<AppState>) {
    start_server_on(state, "127.0.0.1:3456").await;
}

/// 空闲自动关闭阈值：30 分钟无任何请求则优雅退出（释放端口与连接池）
const IDLE_SHUTDOWN_SECS: u64 = 30 * 60;

/// 启动 API Hub HTTP 服务到指定地址（测试可绑定 `127.0.0.1:0`）。
/// 服务空闲超过 IDLE_SHUTDOWN_SECS 后自动优雅关闭，并把 started 复位，
/// 下次使用 API Hub 的命令会再次触发惰性启动（ensure_started）。
pub async fn start_server_on(state: Arc<AppState>, addr: &str) {
    let app = build_router(state.clone());
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[API Hub] Failed to bind {}: {}", addr, e);
            return;
        }
    };
    if let Ok(local) = listener.local_addr() {
        println!("[API Hub] Server started on http://{}", local);
    }
    state.running.store(true, Ordering::SeqCst);
    // 记录启动时刻作为首个活动时间，避免「刚启动就被判定空闲」
    touch_activity(&state);

    // 空闲超时触发优雅关闭：axum::serve(..).with_graceful_shutdown 等待该 future
    let idle_state = state.clone();
    let idle_shutdown = async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let now_ms = now_unix_ms();
            let last = idle_state.last_activity_ms.load(Ordering::Relaxed);
            if last > 0 && now_ms.saturating_sub(last) >= IDLE_SHUTDOWN_SECS * 1000 {
                println!("[API Hub] Idle for {IDLE_SHUTDOWN_SECS}s, shutting down server");
                break;
            }
        }
    };

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(idle_shutdown)
        .await
    {
        eprintln!("[API Hub] Server error: {}", e);
    }
    state.running.store(false, Ordering::SeqCst);
    // 复位 started，允许下次使用再次惰性拉起
    let _ = state
        .started
        .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst);
}

/// 记录一次服务活动（每次 HTTP 请求命中时调用）
fn touch_activity(state: &AppState) {
    state
        .last_activity_ms
        .store(now_unix_ms(), Ordering::Relaxed);
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 构建 Router（供集成测试 / 冒烟示例使用）
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("tauri://localhost"),
            HeaderValue::from_static("http://localhost:1420"), // dev
            HeaderValue::from_static("http://127.0.0.1:1420"), // dev fallback
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
            HeaderName::from_static("x-devnexus-token"),
            HeaderName::from_static("x-goog-api-key"),
        ]);

    // 所有代理端点统一需要访问令牌（含 Ollama，避免本机任意进程盗用 Key）
    // BodyLimit 分级：代理端点 10MB（支持大上下文），health/tags 64KB（轻量）
    let protected = Router::new()
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/responses", post(responses_handler))
        .route("/v1/messages", post(anthropic_messages_handler))
        .route(
            "/v1beta/models/:model_action",
            post(gemini_generate_handler),
        )
        .route("/v1/models", get(list_models_handler))
        .route("/api/chat", post(ollama_chat_handler))
        .route("/api/tags", get(ollama_tags_handler))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_auth,
        ))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024));

    let health = Router::new()
        .route("/health", get(health_handler))
        .layer(DefaultBodyLimit::max(64 * 1024));

    Router::new()
        .merge(health)
        .merge(protected)
        .layer(cors)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            touch_activity_mw,
        ))
        .with_state(state)
}

/// 请求级活动中间件：每次命中任一端点都刷新 last_activity_ms，
/// 供空闲自动关闭逻辑判断「是否仍在使用」。
async fn touch_activity_mw(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    touch_activity(&state);
    next.run(req).await
}

/// 认证中间件：校验 `X-DevNexus-Token`、`Authorization: Bearer <token>`，
/// 并兼容 Gemini 客户端凭据风格（`x-goog-api-key` 头或 `?key=` 查询参数）。
/// 防止本机任意进程/浏览器页面盗用已配置的 API Key 调用上游（消耗用户额度）。
async fn require_auth(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let token = state.auth_token.as_str();
    let ok = req
        .headers()
        .get(HeaderName::from_static("x-devnexus-token"))
        .and_then(|v| v.to_str().ok())
        .map(|v| ct_eq(v, token))
        .or_else(|| {
            req.headers()
                .get(HeaderName::from_static("authorization"))
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(|v| ct_eq(v.trim(), token))
        })
        .or_else(|| {
            req.headers()
                .get(HeaderName::from_static("x-goog-api-key"))
                .and_then(|v| v.to_str().ok())
                .map(|v| ct_eq(v, token))
        })
        .or_else(|| {
            req.uri()
                .query()
                .and_then(|q| q.split('&').find(|p| p.strip_prefix("key=").is_some()))
                .and_then(|p| p.strip_prefix("key="))
                .map(|v| ct_eq(v, token))
        })
        .unwrap_or(false);

    if !ok {
        return error_response(
            401,
            "Unauthorized: missing or invalid X-DevNexus-Token header. The token is shown in the API Hub page.",
        );
    }
    next.run(req).await
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

/// Gemini 客户端端点：/v1beta/models/{model}:generateContent（流式为
/// :streamGenerateContent）。模型名在路径中，注入请求体后走统一管线。
async fn gemini_generate_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(model_action): axum::extract::Path<String>,
    Json(mut body): Json<serde_json::Value>,
) -> Response {
    let Some((model, action)) = model_action.split_once(':') else {
        return error_response(
            404,
            "Invalid Gemini endpoint, expected /v1beta/models/{model}:generateContent",
        );
    };
    if action != "generateContent" && action != "streamGenerateContent" {
        return error_response(404, &format!("Unsupported Gemini method: {action}"));
    }
    if let Some(obj) = body.as_object_mut() {
        obj.insert("model".into(), serde_json::json!(model));
        obj.insert(
            "stream".into(),
            serde_json::json!(action == "streamGenerateContent"),
        );
    }
    handle_unified(state, body, ClientFormat::Gemini).await
}

/// Ollama 客户端端点：/api/chat（NDJSON 流式）。
async fn ollama_chat_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    handle_unified(state, body, ClientFormat::Ollama).await
}

/// Ollama 模型发现端点：/api/tags，聚合全部启用 Provider 的模型。
async fn ollama_tags_handler(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let providers = state.providers.read().await;
    let models: Vec<serde_json::Value> = providers
        .iter()
        .filter(|p| p.enabled)
        .flat_map(|p| {
            p.models.iter().map(move |m| {
                serde_json::json!({
                    "name": m,
                    "model": m,
                    "size": 0,
                    "digest": "",
                    "modified_at": "",
                    "details": {
                        "family": p.protocol.as_str(),
                        "parameter_size": "",
                        "quantization_level": "",
                    },
                })
            })
        })
        .collect();
    Json(serde_json::json!({ "models": models }))
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
    //    转换失败源于客户端输入（请求体非法/格式不支持）→ 400，而非 5xx
    let internal_req = match client_request_to_internal(client, &body) {
        Ok(b) => b,
        Err(e) => return error_response(400, &e),
    };

    // 2. 内部格式 → Provider 协议格式
    //    转换失败源于客户端输入解析（如非法 OpenAI 请求）→ 400
    let upstream_body = match internal_request_to_provider(&internal_req, &route) {
        Ok(b) => b,
        Err(e) => return error_response(400, &e),
    };

    let endpoint = upstream_endpoint(&route.provider.protocol, &route.model, is_streaming);

    if is_streaming {
        return handle_streaming(&state, &route, &endpoint, upstream_body, client).await;
    }

    // 3. 转发到上游
    let (resp_body, _status) =
        match super::forwarder::forward_request(&state, &route.provider, &endpoint, upstream_body)
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

/// 生成上游端点路径。Gemini 的路径含模型名（流式还带 ?alt=sse），
/// 其余协议沿用 ApiProtocol::endpoint() 的静态路径。
fn upstream_endpoint(protocol: &ApiProtocol, model: &str, streaming: bool) -> String {
    match protocol {
        ApiProtocol::Gemini => {
            if streaming {
                format!("/v1beta/models/{model}:streamGenerateContent?alt=sse")
            } else {
                format!("/v1beta/models/{model}:generateContent")
            }
        }
        _ => protocol.endpoint().to_string(),
    }
}

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
        ClientFormat::Gemini => {
            let mut req = super::transform::gemini::gemini_to_openai_req(body)?;
            // 流式与否由路径（:streamGenerateContent）决定，转换器不感知；
            // gemini_generate_handler 已注入 body.stream，这里带回内部请求
            req.stream = body.get("stream").and_then(|s| s.as_bool());
            serde_json::to_value(req).map_err(|e| format!("Serialization error: {}", e))
        }
        ClientFormat::Ollama => {
            let req = super::transform::ollama::ollama_to_openai_req(body)?;
            serde_json::to_value(req).map_err(|e| format!("Serialization error: {}", e))
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
        ApiProtocol::OpenAIResponses => Ok(super::transform::responses::chat_request_to_responses(
            internal,
        )),
        ApiProtocol::Anthropic => {
            let oai: super::types::OpenAIChatRequest = serde_json::from_value(internal.clone())
                .map_err(|e| format!("Invalid OpenAI request: {}", e))?;
            let anth = super::transform::anthropic::openai_to_anthropic(&oai);
            serde_json::to_value(anth).map_err(|e| format!("Serialization error: {}", e))
        }
        ApiProtocol::Gemini => {
            let oai: super::types::OpenAIChatRequest = serde_json::from_value(internal.clone())
                .map_err(|e| format!("Invalid OpenAI request: {}", e))?;
            Ok(super::transform::gemini::openai_to_gemini(&oai))
        }
        ApiProtocol::Ollama => {
            let oai: super::types::OpenAIChatRequest = serde_json::from_value(internal.clone())
                .map_err(|e| format!("Invalid OpenAI request: {}", e))?;
            Ok(super::transform::ollama::openai_to_ollama_req(&oai))
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
        ApiProtocol::Gemini => Ok(super::transform::gemini::gemini_to_openai(
            &format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
            &route.model,
            resp,
        )),
        ApiProtocol::Ollama => Ok(super::transform::ollama::ollama_to_openai(
            &format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
            resp,
        )),
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
        ClientFormat::Gemini => Ok(super::transform::gemini::openai_to_gemini_response(
            internal,
        )),
        ClientFormat::Ollama => Ok(super::transform::ollama::openai_to_ollama_response(
            internal,
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

    // 两段式转换管线：供应商格式 → 内部 OpenAIChat → 客户端格式。
    // 任一段与内部同格式则该段直通；两段都直通（同协议）则字节级透传。
    let (dir_in, dir_out) = determine_stream_pipeline(route.provider.protocol, client);

    // 在上游字节流上包一层 usage 捕获：流结束后回填 token 用量到日志
    let byte_stream = capture_usage_stream(resp.bytes_stream(), state.clone(), log_id);

    if dir_in.is_some() || dir_out.is_some() {
        // Cross-protocol: transform each line through the two-stage pipeline
        let transformed = transform_byte_stream(byte_stream, dir_in, dir_out, route.model.clone());
        let body = axum::body::Body::from_stream(transformed);

        let content_type = "text/event-stream";

        Response::builder()
            .status(status)
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .header("Content-Type", content_type)
            .body(body)
            .unwrap_or_else(|_| error_response(500, "Stream build error"))
    } else {
        // Same protocol: passthrough bytes directly (zero overhead)
        let stream = byte_stream.map_err(|e| std::io::Error::other(format!("Stream error: {}", e)));
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

/// 两段式流式管线：返回 (供应商→内部, 内部→客户端) 两个方向。
/// 任一段为 None 表示该段直通（该侧本身就是内部 OpenAIChat 格式）。
/// 两侧都 None（同协议）由调用方走字节级透传。
fn determine_stream_pipeline(
    provider_protocol: ApiProtocol,
    client: ClientFormat,
) -> (Option<StreamDirection>, Option<StreamDirection>) {
    let client_protocol = match client {
        ClientFormat::OpenAIChat => ApiProtocol::OpenAIChat,
        ClientFormat::OpenAIResponses => ApiProtocol::OpenAIResponses,
        ClientFormat::Anthropic => ApiProtocol::Anthropic,
        ClientFormat::Gemini => ApiProtocol::Gemini,
        ClientFormat::Ollama => ApiProtocol::Ollama,
    };

    if client_protocol == provider_protocol {
        return (None, None); // 同协议：字节级透传
    }

    // 阶段一：供应商流 → 内部 OpenAIChat 流
    let dir_in = match provider_protocol {
        ApiProtocol::OpenAIChat => None,
        ApiProtocol::Anthropic => Some(StreamDirection::AnthropicToOpenAIChat),
        ApiProtocol::OpenAIResponses => Some(StreamDirection::ResponsesToOpenAIChat),
        ApiProtocol::Gemini => Some(StreamDirection::GeminiToOpenAIChat),
        ApiProtocol::Ollama => Some(StreamDirection::OllamaToOpenAIChat),
    };
    // 阶段二：内部 OpenAIChat 流 → 客户端格式
    let dir_out = match client_protocol {
        ApiProtocol::OpenAIChat => None,
        ApiProtocol::Anthropic => Some(StreamDirection::OpenAIChatToAnthropic),
        ApiProtocol::OpenAIResponses => Some(StreamDirection::OpenAIChatToResponses),
        ApiProtocol::Gemini => Some(StreamDirection::OpenAIChatToGemini),
        ApiProtocol::Ollama => Some(StreamDirection::OpenAIChatToOllama),
    };
    (dir_in, dir_out)
}

/// 包装上游字节流：数据原样透传，同时扫描 SSE 行提取 usage token 用量，
/// 流结束后异步回填到请求日志（解决流式请求 token 统计为 0 的问题）。
/// 使用索引游标避免 `drain(..=pos)` 的 O(n) 搬移，按行触发 `extract_usage`。
fn capture_usage_stream(
    byte_stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
    state: AppState,
    log_id: String,
) -> impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static {
    async_stream::stream! {
        use futures_util::StreamExt;
        // 扫描缓冲上限：防御异常上游把单行撑得过大占用内存
        const MAX_SCAN_BUF: usize = 256 * 1024;
        let mut scan_buf: Vec<u8> = Vec::with_capacity(4096);
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;

        futures_util::pin_mut!(byte_stream);

        while let Some(chunk_result) = byte_stream.next().await {
            if let Ok(ref chunk) = chunk_result {
                if scan_buf.len() < MAX_SCAN_BUF {
                    scan_buf.extend_from_slice(chunk);
                    // 按换行切分，避免 `String::drain` 的重复搬移
                    let mut start = 0usize;
                    while let Some(rel) = scan_buf[start..].iter().position(|&b| b == b'\n') {
                        let end = start + rel;
                        let line = String::from_utf8_lossy(&scan_buf[start..end]);
                        let trimmed = line.trim_end_matches('\r');
                        extract_usage_from_sse_line(trimmed, &mut input_tokens, &mut output_tokens);
                        start = end + 1;
                    }
                    if start > 0 {
                        scan_buf.drain(0..start);
                    }
                }
            }
            yield chunk_result;
        }

        // 处理残余缓冲，然后回填 token 用量
        if !scan_buf.is_empty() {
            let tail = String::from_utf8_lossy(&scan_buf);
            extract_usage_from_sse_line(tail.trim(), &mut input_tokens, &mut output_tokens);
        }
        if input_tokens > 0 || output_tokens > 0 {
            tokio::spawn(async move {
                super::usage::update_log_tokens(&state, &log_id, input_tokens, output_tokens).await;
            });
        }
    }
}

/// 从单行中提取 usage 字段。兼容：
/// - SSE data: 行（OpenAI Chat / Responses / Anthropic / Gemini）
/// - 裸 JSON 行（Ollama NDJSON：prompt_eval_count / eval_count）
fn extract_usage_from_sse_line(line: &str, input: &mut u64, output: &mut u64) {
    let data = match line.strip_prefix("data:") {
        Some(d) => d.trim(),
        None => line.trim(),
    };
    // 快速路径：不含任何用量字段特征的行直接跳过，避免逐 chunk 反序列化
    if data.is_empty()
        || data == "[DONE]"
        || !(data.contains("\"usage\"")
            || data.contains("usageMetadata")
            || data.contains("prompt_eval_count"))
    {
        return;
    }
    let json: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    // usage 可能位于顶层（OpenAI Chat / Anthropic message_delta）、
    // message 下（Anthropic message_start）或 response 下（Responses response.completed）
    // Ollama：计数在顶层
    if let Some(v) = json.get("prompt_eval_count").and_then(|v| v.as_u64()) {
        *input = (*input).max(v);
    }
    if let Some(v) = json.get("eval_count").and_then(|v| v.as_u64()) {
        *output = (*output).max(v);
    }

    let usage = json
        .get("usage")
        .or_else(|| json.get("usageMetadata"))
        .or_else(|| json.get("message").and_then(|m| m.get("usage")))
        .or_else(|| json.get("response").and_then(|r| r.get("usage")));
    let Some(usage) = usage else { return };

    let in_val = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .or_else(|| usage.get("promptTokenCount"))
        .and_then(|v| v.as_u64());
    let out_val = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .or_else(|| usage.get("candidatesTokenCount"))
        .and_then(|v| v.as_u64());
    if let Some(v) = in_val {
        *input = (*input).max(v);
    }
    if let Some(v) = out_val {
        *output = (*output).max(v);
    }
}

/// 两段式行转换流：供应商行先经 dir_in 转为内部 OpenAIChat 行，
/// 再经 dir_out 转为客户端格式；任一段 None 表示直通该段。
/// 缓冲按行切分，预分配避免频繁扩容。
fn transform_byte_stream(
    byte_stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
    dir_in: Option<StreamDirection>,
    dir_out: Option<StreamDirection>,
    model: String,
) -> impl futures_util::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send + 'static {
    async_stream::stream! {
        use futures_util::StreamExt;
        let mut state_in = StreamState::default();
        let mut state_out = StreamState::default();
        // Gemini/Ollama 流不携带模型名或字段不同，预填路由模型
        state_in.model = model.clone();
        state_out.model = model;
        if dir_in == Some(StreamDirection::GeminiToOpenAIChat) && state_in.id.is_empty() {
            state_in.id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());
        }
        let mut buffer: Vec<u8> = Vec::with_capacity(8192);

        futures_util::pin_mut!(byte_stream);

        while let Some(chunk_result) = byte_stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    yield Err(std::io::Error::other(format!("Upstream error: {}", e)));
                    return;
                }
            };

            buffer.extend_from_slice(&chunk);

            // 按行喂入两段管线，使用索引游标避免重复 `drain` 搬移开销
            let mut start = 0usize;
            while let Some(rel) = buffer[start..].iter().position(|&b| b == b'\n') {
                let end = start + rel;
                let line = String::from_utf8_lossy(&buffer[start..end]);
                let trimmed = line.trim_end_matches(['\r', '\n']).to_string();

                // 阶段一：供应商 → 内部（None 直通）
                let internal_lines: Vec<String> = match dir_in {
                    Some(d) => transform_sse_line(d, &trimmed, &mut state_in),
                    None => vec![trimmed],
                };
                // 阶段二：内部 → 客户端（None 直通）
                for l in internal_lines {
                    let out_lines: Vec<String> = match dir_out {
                        Some(d) => transform_sse_line(d, &l, &mut state_out),
                        None => vec![l],
                    };
                    for mut ol in out_lines {
                        ol.push('\n');
                        yield Ok(bytes::Bytes::from(ol));
                    }
                }
                start = end + 1;
            }
            if start > 0 {
                buffer.drain(0..start);
            }
        }

        // 处理最后一行（无换行结尾的残行）
        if !buffer.is_empty() {
            let tail = String::from_utf8_lossy(&buffer).trim_end_matches(['\r', '\n']).to_string();
            if !tail.is_empty() {
                let internal_lines: Vec<String> = match dir_in {
                    Some(d) => transform_sse_line(d, &tail, &mut state_in),
                    None => vec![tail],
                };
                for l in internal_lines {
                    let out_lines: Vec<String> = match dir_out {
                        Some(d) => transform_sse_line(d, &l, &mut state_out),
                        None => vec![l],
                    };
                    for mut ol in out_lines {
                        ol.push('\n');
                        yield Ok(bytes::Bytes::from(ol));
                    }
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

/// 常量时间字符串比较，缓解针对本地服务访问令牌的时序侧信道。
/// 令牌为固定长度且仅本机可见，长度提前返回可接受。
fn ct_eq(a: &str, b: &str) -> bool {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    ab.len() == bb.len() && ab.iter().zip(bb).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn error_response(status: u16, message: &str) -> Response {
    // 限长，避免大错误体回写与前端渲染卡顿
    const MAX_MSG: usize = 500;
    let msg = if message.len() > MAX_MSG {
        let mut cut = MAX_MSG;
        while !message.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}…", &message[..cut])
    } else {
        message.to_string()
    };
    let body = serde_json::json!({
        "error": {
            "message": msg,
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
