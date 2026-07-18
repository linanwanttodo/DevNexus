//! API Hub 端到端测试：mock 上游 + 真实 axum hub

use super::provider;
use super::server::build_router;
use super::types::{ApiProtocol, AppState, Provider};
use axum::{
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;

async fn mock_openai_chat(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("mock-gpt")
        .to_string();
    Json(serde_json::json!({
        "id": "chatcmpl-mock",
        "object": "chat.completion",
        "created": 1,
        "model": model,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "hello-from-openai-mock"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 3, "completion_tokens": 5, "total_tokens": 8}
    }))
}

async fn mock_anthropic_messages(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("mock-claude")
        .to_string();
    // 验证 OpenAI→Anthropic 转换后至少有 messages
    assert!(
        body.get("messages").and_then(|m| m.as_array()).is_some(),
        "anthropic mock expected messages field, got {}",
        body
    );
    Json(serde_json::json!({
        "id": "msg_mock",
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [{"type": "text", "text": "hello-from-anthropic-mock"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 4, "output_tokens": 6}
    }))
}

async fn mock_gemini(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    assert!(
        body.get("contents").is_some(),
        "gemini mock expected contents, got {}",
        body
    );
    Json(serde_json::json!({
        "candidates": [{
            "content": {"parts": [{"text": "hello-from-gemini-mock"}], "role": "model"},
            "finishReason": "STOP"
        }],
        "usageMetadata": {
            "promptTokenCount": 2,
            "candidatesTokenCount": 7
        }
    }))
}

async fn mock_openai_responses(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    assert!(
        body.get("input").is_some(),
        "responses mock expected input, got {}",
        body
    );
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("mock-resp")
        .to_string();
    Json(serde_json::json!({
        "id": "resp_mock",
        "object": "response",
        "model": model,
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "hello-from-responses-mock"}],
            "status": "completed"
        }],
        "usage": {"input_tokens": 1, "output_tokens": 2, "total_tokens": 3}
    }))
}

async fn spawn_mock_upstream() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/v1/chat/completions", post(mock_openai_chat))
        .route("/v1/messages", post(mock_anthropic_messages))
        // 路径段为 `gemini-mock:generateContent`（冒号在段内）
        .route("/v1/models/:model_action", post(mock_gemini))
        .route("/v1/responses", post(mock_openai_responses))
        .route("/health", get(|| async { "ok" }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, handle)
}

fn test_state(upstream: &str) -> AppState {
    let db = Arc::new(std::sync::Mutex::new(None));
    let state = AppState {
        providers: Arc::new(std::sync::RwLock::new(Vec::new())),
        request_logs: Arc::new(std::sync::RwLock::new(Vec::new())),
        db,
    };

    // 不走 SQLite，直接内存注入 provider
    let providers = vec![
        Provider {
            id: "p-openai".into(),
            name: "Mock OpenAI".into(),
            protocol: ApiProtocol::OpenAIChat,
            base_url: format!("http://{}", upstream),
            api_key: "sk-test".into(),
            models: vec!["mock-gpt".into()],
            model_aliases: Default::default(),
            enabled: true,
            created_at: 0,
        },
        Provider {
            id: "p-anth".into(),
            name: "Mock Anthropic".into(),
            protocol: ApiProtocol::Anthropic,
            base_url: format!("http://{}", upstream),
            api_key: "anth-test".into(),
            models: vec!["mock-claude".into()],
            model_aliases: Default::default(),
            enabled: true,
            created_at: 0,
        },
        Provider {
            id: "p-gem".into(),
            name: "Mock Gemini".into(),
            protocol: ApiProtocol::Gemini,
            base_url: format!("http://{}", upstream),
            api_key: "gem-test".into(),
            models: vec!["gemini-mock".into()],
            model_aliases: Default::default(),
            enabled: true,
            created_at: 0,
        },
        Provider {
            id: "p-resp".into(),
            name: "Mock Responses".into(),
            protocol: ApiProtocol::OpenAIResponses,
            base_url: format!("http://{}", upstream),
            api_key: "sk-resp".into(),
            models: vec!["mock-resp".into()],
            model_aliases: Default::default(),
            enabled: true,
            created_at: 0,
        },
    ];

    *state.providers.write().unwrap() = providers;
    // 绕过 init_db；usage log 可能写 db，forwarder 在 db 为 None 时应安全
    let _ = provider::init_db(&state);
    state
}

async fn spawn_hub(state: AppState) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = build_router(Arc::new(state));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // 给一点时间监听就绪
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (addr, handle)
}

async fn post_json(url: &str, body: serde_json::Value) -> (u16, serde_json::Value) {
    let client = reqwest::Client::new();
    let resp = client.post(url).json(&body).send().await.unwrap();
    let status = resp.status().as_u16();
    let json = resp.json().await.unwrap_or(serde_json::json!({}));
    (status, json)
}

#[tokio::test]
async fn e2e_health_and_models() {
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let health: serde_json::Value = reqwest::get(format!("http://{}/health", hub))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["status"], "ok");

    let models: serde_json::Value = reqwest::get(format!("http://{}/v1/models", hub))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let data = models["data"].as_array().unwrap();
    assert!(
        data.len() >= 4,
        "expected registered models, got {:?}",
        data
    );
}

#[tokio::test]
async fn e2e_openai_to_openai_passthrough() {
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let (status, body) = post_json(
        &format!("http://{}/v1/chat/completions", hub),
        serde_json::json!({
            "model": "mock-gpt",
            "messages": [{"role": "user", "content": "hi"}]
        }),
    )
    .await;
    assert_eq!(status, 200, "{:?}", body);
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "hello-from-openai-mock"
    );
}

#[tokio::test]
async fn e2e_openai_client_to_anthropic_upstream() {
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let (status, body) = post_json(
        &format!("http://{}/v1/chat/completions", hub),
        serde_json::json!({
            "model": "mock-claude",
            "messages": [
                {"role": "system", "content": "be short"},
                {"role": "user", "content": "hi"}
            ],
            "max_tokens": 64
        }),
    )
    .await;
    assert_eq!(status, 200, "{:?}", body);
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "hello-from-anthropic-mock"
    );
    assert_eq!(body["usage"]["prompt_tokens"], 4);
    assert_eq!(body["usage"]["completion_tokens"], 6);
}

#[tokio::test]
async fn e2e_anthropic_client_to_openai_upstream() {
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let (status, body) = post_json(
        &format!("http://{}/v1/messages", hub),
        serde_json::json!({
            "model": "mock-gpt",
            "max_tokens": 32,
            "messages": [{"role": "user", "content": "hello"}]
        }),
    )
    .await;
    assert_eq!(status, 200, "{:?}", body);
    assert_eq!(body["type"], "message");
    assert_eq!(body["content"][0]["text"], "hello-from-openai-mock");
}

#[tokio::test]
async fn e2e_openai_client_to_gemini_upstream() {
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let (status, body) = post_json(
        &format!("http://{}/v1/chat/completions", hub),
        serde_json::json!({
            "model": "gemini-mock",
            "messages": [{"role": "user", "content": "hi gemini"}]
        }),
    )
    .await;
    assert_eq!(status, 200, "{:?}", body);
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "hello-from-gemini-mock"
    );
}

#[tokio::test]
async fn e2e_openai_client_to_responses_upstream() {
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let (status, body) = post_json(
        &format!("http://{}/v1/chat/completions", hub),
        serde_json::json!({
            "model": "mock-resp",
            "messages": [
                {"role": "system", "content": "sys"},
                {"role": "user", "content": "ping"}
            ]
        }),
    )
    .await;
    assert_eq!(status, 200, "{:?}", body);
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "hello-from-responses-mock"
    );
}

#[tokio::test]
async fn e2e_missing_model_returns_400() {
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let (status, body) = post_json(
        &format!("http://{}/v1/chat/completions", hub),
        serde_json::json!({
            "messages": [{"role": "user", "content": "x"}]
        }),
    )
    .await;
    assert_eq!(status, 400, "{:?}", body);
}
