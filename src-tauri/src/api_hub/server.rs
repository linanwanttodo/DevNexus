use super::router::{route_by_model, RouteResult};
use super::types::{ApiProtocol, AppState};
use axum::{
    extract::State,
    http::Method,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::TryStreamExt;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// 客户端请求所使用的格式（由命中的入口端点决定）
#[derive(Clone, Copy)]
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
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/responses", post(responses_handler))
        .route("/v1/messages", post(anthropic_messages_handler))
        .route("/v1/models", get(list_models_handler))
        .layer(cors)
        .with_state(state);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => {
            if let Ok(local) = l.local_addr() {
                println!("[API Hub] Server started on http://{}", local);
            }
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
        .layer(cors)
        .with_state(state)
}

// ── Health ────────────────────────────────────────────────────

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "server": "DevNexus API Hub",
        "port": 3456,
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

    let route = match route_by_model(&state, &model) {
        Some(r) => r,
        None => {
            return error_response(404, &format!("No provider found for model '{}'", model));
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

    // Gemini 等协议的 endpoint 含 {model} 占位符，用路由模型填充
    let endpoint = route
        .provider
        .protocol
        .endpoint()
        .replace("{model}", &route.model);

    // Gemini 请求体无 model 字段，注入以便 forwarder 构建 URL / 记日志
    let mut upstream_body = upstream_body;
    if !upstream_body
        .get("model")
        .and_then(|m| m.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        upstream_body["model"] = serde_json::Value::String(route.model.clone());
    }

    if is_streaming {
        return handle_streaming(&state, &route, &endpoint, upstream_body).await;
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
    let providers = state.providers.read().unwrap();
    let mut models: Vec<serde_json::Value> = Vec::new();

    for p in providers.iter() {
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
        ApiProtocol::OpenAIChat | ApiProtocol::Ollama => Ok(internal.clone()),
        ApiProtocol::OpenAIResponses => {
            // 请求方向：chat → responses request（不是响应转换）
            Ok(super::transform::responses::chat_request_to_responses(
                internal,
            ))
        }
        ApiProtocol::Anthropic => {
            let oai: super::types::OpenAIChatRequest = serde_json::from_value(internal.clone())
                .map_err(|e| format!("Invalid OpenAI request: {}", e))?;
            let anth = super::transform::anthropic::openai_to_anthropic(&oai);
            serde_json::to_value(anth).map_err(|e| format!("Serialization error: {}", e))
        }
        ApiProtocol::Gemini => Ok(super::transform::gemini::openai_to_gemini(internal)),
    }
}

/// Provider 响应 → 内部 OpenAIChat 格式
fn provider_response_to_internal(
    resp: &serde_json::Value,
    route: &RouteResult,
) -> Result<serde_json::Value, String> {
    match route.provider.protocol {
        ApiProtocol::OpenAIChat | ApiProtocol::Ollama => Ok(resp.clone()),
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
            resp,
            &route.model,
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

/// 处理流式请求（SSE 透传上游字节，不做响应格式转换）
async fn handle_streaming(
    state: &AppState,
    route: &RouteResult,
    endpoint: &str,
    upstream_body: serde_json::Value,
) -> Response {
    let resp =
        match super::forwarder::forward_streaming(state, &route.provider, endpoint, upstream_body)
            .await
        {
            Ok(r) => r,
            Err(e) => return error_response(502, &e),
        };

    let status = axum::http::StatusCode::from_u16(resp.status().as_u16())
        .unwrap_or(axum::http::StatusCode::OK);
    let headers = resp.headers().clone();

    let stream = resp.bytes_stream().map_err(|e| {
        eprintln!("[API Hub] Stream error: {}", e);
        std::io::Error::other(e)
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
