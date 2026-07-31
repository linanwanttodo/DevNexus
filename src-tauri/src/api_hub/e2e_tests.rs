//! API Hub 端到端测试：mock 上游 + 真实 axum hub

use super::crypto::ApiKeyCipher;
use super::provider;
use super::server::build_router;
use super::types::{ApiProtocol, AppState, Provider};
use axum::{
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

async fn mock_openai_chat(Json(body): Json<serde_json::Value>) -> axum::response::Response {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("mock-gpt")
        .to_string();

    // 如果是流式请求，返回 SSE 格式
    if body
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false)
    {
        let sse = format!(
            "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            serde_json::json!({"id":"chatcmpl-sse","object":"chat.completion.chunk","created":1,"model":model,"choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}),
            serde_json::json!({"id":"chatcmpl-sse","object":"chat.completion.chunk","created":1,"model":model,"choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}),
            serde_json::json!({"id":"chatcmpl-sse","object":"chat.completion.chunk","created":1,"model":model,"choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}),
            serde_json::json!({"id":"chatcmpl-sse","object":"chat.completion.chunk","created":1,"model":model,"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
        );
        let body = axum::body::Body::from(sse);
        return axum::response::Response::builder()
            .header("content-type", "text/event-stream")
            .body(body)
            .unwrap();
    }

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
    .into_response()
}

async fn mock_anthropic_messages(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("mock-claude")
        .to_string();
    assert!(
        body.get("messages").and_then(|m| m.as_array()).is_some(),
        "anthropic mock expected messages field, got {}",
        body
    );

    // 如果流式，返回 SSE Anthropic 格式
    if body
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false)
    {
        let sse = format!(
            "event: message_start\ndata: {}\n\nevent: content_block_start\ndata: {}\n\nevent: content_block_delta\ndata: {}\n\nevent: content_block_delta\ndata: {}\n\nevent: content_block_stop\ndata: {}\n\nevent: message_delta\ndata: {}\n\nevent: message_stop\ndata: {}\n\n",
            serde_json::json!({"type":"message_start","message":{"id":"msg_sse","type":"message","role":"assistant","model":model,"content":[],"stop_reason":null,"usage":{"input_tokens":5,"output_tokens":0}}}),
            serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}),
            serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" anthropic"}}),
            serde_json::json!({"type":"content_block_stop","index":0}),
            serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}),
            serde_json::json!({"type":"message_stop"}),
        );
        let body = axum::body::Body::from(sse);
        return axum::response::Response::builder()
            .header("content-type", "text/event-stream")
            .body(body)
            .unwrap()
            .into_response();
    }

    Json(serde_json::json!({
        "id": "msg_mock",
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [{"type": "text", "text": "hello-from-anthropic-mock"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 4, "output_tokens": 6}
    }))
    .into_response()
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
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    provider::init_db_sync(&conn).unwrap();

    let providers = vec![
        Provider {
            id: "p-openai".into(),
            name: "Mock OpenAI".into(),
            protocol: ApiProtocol::OpenAIChat,
            base_url: format!("http://{}", upstream),
            api_key: "sk-test".into(),
            models: vec!["mock-gpt".into(), "mock-sse".into()],
            model_aliases: Default::default(),
            model_context_lengths: Default::default(),
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
            model_context_lengths: Default::default(),
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
            model_context_lengths: Default::default(),
            enabled: true,
            created_at: 0,
        },
    ];

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    AppState {
        providers: Arc::new(tokio::sync::RwLock::new(providers)),
        request_logs: Arc::new(tokio::sync::RwLock::new(VecDeque::new())),
        db: Arc::new(tokio::sync::Mutex::new(Some(conn))),
        http_client,
        running: Arc::new(AtomicBool::new(false)),
        api_key_cipher: Arc::new(ApiKeyCipher::from_key([7u8; 32], true)),
    }
}

async fn spawn_hub(state: AppState) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = build_router(Arc::new(state));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
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
        data.len() >= 3,
        "expected registered models, got {:?}",
        data
    );
}

#[tokio::test]
async fn e2e_cors_blocks_untrusted_origin() {
    // 回归测试：CORS 白名单化后，任意网页 (evil origin) 不应拿到 ACAO 头，
    // 防止跨站 JS 读取 hub 响应（避免 API Key 被盗用）。
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/health", hub))
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    let allow_origin = resp
        .headers()
        .get("access-control-allow-origin")
        .map(|v| v.to_str().unwrap_or_default().to_string());
    assert!(
        allow_origin.is_none(),
        "untrusted origin must not be allowed, got access-control-allow-origin: {:?}",
        allow_origin
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

#[tokio::test]
async fn e2e_streaming_chat() {
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/v1/chat/completions", hub))
        .json(&serde_json::json!({
            "model": "mock-sse",
            "messages": [{"role": "user", "content": "hi"}],
            "stream": true
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap_or_default();
    assert!(!body.is_empty(), "SSE body should not be empty");
    assert!(
        body.contains("data: [DONE]"),
        "SSE should end with [DONE], got: {}",
        &body[..body.len().min(200)]
    );
    assert!(body.contains("Hello"), "SSE should contain Hello");
}

#[tokio::test]
async fn e2e_streaming_cross_protocol_openai_to_anthropic() {
    // OpenAI Chat 客户端请求，路由到 Anthropic 上游（流式），hub 应转换格式
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/v1/chat/completions", hub))
        .json(&serde_json::json!({
            "model": "mock-claude",
            "messages": [{"role": "user", "content": "hi"}],
            "stream": true
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap_or_default();
    // 转换后应该是 OpenAI Chat 格式的 SSE，包含 data: [DONE]
    assert!(
        body.contains("data: [DONE]"),
        "Converted SSE should end with [DONE], got: {}",
        &body[..body.len().min(300)]
    );
    // 应该包含 chat.completion.chunk 格式
    assert!(
        body.contains("chat.completion.chunk"),
        "Converted SSE should be in OpenAI Chat format, got: {}",
        &body[..body.len().min(300)]
    );
    // 应包含转换后的内容
    assert!(
        body.contains("Hello") || body.contains("anthropic"),
        "Converted SSE should contain content"
    );
}

#[tokio::test]
async fn e2e_streaming_cross_protocol_anthropic_to_responses_unsupported() {
    // 回归测试（C3）：Anthropic 上游 → Responses 客户端 的流式级联转换未实现，
    // 必须显式返回 422，而不是静默降级成 OpenAI Chat 流（chat.completion.chunk）。
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let (status, body) = post_json(
        &format!("http://{}/v1/responses", hub),
        serde_json::json!({
            "model": "mock-claude",
            "input": "hi",
            "stream": true
        }),
    )
    .await;

    assert_eq!(status, 422, "expected 422, body: {:?}", body);
    let msg = body["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(
        msg.contains("is not supported")
            && msg.contains("anthropic")
            && msg.contains("openai_responses"),
        "error message should mention the unsupported conversion, got: {}",
        msg
    );
}

#[tokio::test]
async fn e2e_streaming_cross_protocol_responses_to_anthropic_unsupported() {
    // 回归测试（C3）：Responses 上游 → Anthropic 客户端 的流式级联转换未实现，
    // 必须显式返回 422，而不是静默降级成 OpenAI Chat 流（chat.completion.chunk）。
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let (status, body) = post_json(
        &format!("http://{}/v1/messages", hub),
        serde_json::json!({
            "model": "mock-resp",
            "max_tokens": 32,
            "messages": [{"role": "user", "content": "hello"}],
            "stream": true
        }),
    )
    .await;

    assert_eq!(status, 422, "expected 422, body: {:?}", body);
    let msg = body["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(
        msg.contains("is not supported")
            && msg.contains("openai_responses")
            && msg.contains("anthropic"),
        "error message should mention the unsupported conversion, got: {}",
        msg
    );
}

#[tokio::test]
async fn e2e_unmatched_model_returns_404() {
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let (hub, _h) = spawn_hub(state).await;

    let (status, _body) = post_json(
        &format!("http://{}/v1/chat/completions", hub),
        serde_json::json!({
            "model": "nonexistent-model-xyz",
            "messages": [{"role": "user", "content": "hi"}]
        }),
    )
    .await;
    // 不再兜底路由，未匹配模型应返回 404
    assert_eq!(status, 404);
}

#[tokio::test]
async fn e2e_streaming_log_tokens_backfilled() {
    // 回归测试（C1）：流式请求结束后，token 用量必须回填到请求日志。
    // 此前 log_request 的 INSERT 是 fire-and-forget，UPDATE 可能先于 INSERT 提交，
    // 导致匹配 0 行、流式 token 永久丢失。
    let (up_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&up_addr.to_string());
    let state_check = state.clone();
    let (hub, _h) = spawn_hub(state).await;

    // Anthropic mock 的 SSE 自带 usage（input_tokens=5, output_tokens=2），
    // 走跨协议流式路径以触发 usage 捕获
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/v1/chat/completions", hub))
        .json(&serde_json::json!({
            "model": "mock-claude",
            "messages": [{"role": "user", "content": "hi"}],
            "stream": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap_or_default();
    assert!(
        body.contains("data: [DONE]"),
        "SSE should end with [DONE], got: {}",
        &body[..body.len().min(200)]
    );

    // 流结束后 update_log_tokens 在后台异步执行，轮询等待回填完成
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let (input, output) = loop {
        let logs = super::usage::get_logs(&state_check, 50, 0).await;
        let streamed = logs.iter().find(|l| l.is_streaming && l.model == "mock-claude");
        if let Some(l) = streamed {
            if l.output_tokens > 0 {
                break (l.input_tokens, l.output_tokens);
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "streaming log tokens never backfilled: {:?}",
            logs
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };

    // Anthropic mock 的 usage：input_tokens=5, output_tokens=2
    assert_eq!(input, 5, "input_tokens should be backfilled from Anthropic mock");
    assert_eq!(output, 2, "output_tokens should be backfilled from Anthropic mock");
}

#[tokio::test]
async fn e2e_provider_duplicate_name_rejected_and_not_persisted() {
    // 回归测试（C2）：重名 Provider（含大小写差异）第二次添加必须返回 Err 且无副作用，
    // 模拟重启（重新 load_providers_from_db_sync）后仍只有一条。
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    provider::init_db_sync(&conn).unwrap();

    let http_client = reqwest::Client::new();
    let state = AppState {
        providers: Arc::new(tokio::sync::RwLock::new(vec![])),
        request_logs: Arc::new(tokio::sync::RwLock::new(VecDeque::new())),
        db: Arc::new(tokio::sync::Mutex::new(Some(conn))),
        http_client,
        running: Arc::new(AtomicBool::new(false)),
        api_key_cipher: Arc::new(ApiKeyCipher::from_key([7u8; 32], true)),
    };

    let mk = |name: &str| Provider {
        id: String::new(), // add_provider 会自动生成 UUID
        name: name.into(),
        protocol: ApiProtocol::OpenAIChat,
        base_url: "http://127.0.0.1:1".into(),
        api_key: "sk-test".into(),
        models: vec!["mock-gpt".into()],
        model_aliases: Default::default(),
        model_context_lengths: Default::default(),
        enabled: true,
        created_at: 0,
    };

    // 第一次添加成功
    super::provider::add_provider(&state, mk("OpenAI"))
        .await
        .expect("first add should succeed");
    assert_eq!(state.providers.read().await.len(), 1);

    // 第二次同名（大小写不同）必须失败
    let err = super::provider::add_provider(&state, mk("openai")).await;
    assert!(
        err.is_err(),
        "duplicate provider (case-insensitive) should be rejected, got {:?}",
        err
    );

    // 内存中仍只有一条
    assert_eq!(state.providers.read().await.len(), 1);

    // 模拟重启：重新从 DB 加载，仍只有一条
    let reloaded = {
        let db = state.db.lock().await;
        provider::load_providers_from_db_sync(db.as_ref().unwrap(), &state.api_key_cipher)
    };
    assert_eq!(reloaded.len(), 1, "no duplicate after simulated restart");
    assert_eq!(reloaded[0].name, "OpenAI");
}

/// 构造一个使用内存 SQLite 的 AppState（与 e2e_provider_duplicate_name_rejected_and_not_persisted 一致）
fn provider_test_state() -> AppState {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    provider::init_db_sync(&conn).unwrap();

    let http_client = reqwest::Client::new();
    AppState {
        providers: Arc::new(tokio::sync::RwLock::new(vec![])),
        request_logs: Arc::new(tokio::sync::RwLock::new(VecDeque::new())),
        db: Arc::new(tokio::sync::Mutex::new(Some(conn))),
        http_client,
        running: Arc::new(AtomicBool::new(false)),
        api_key_cipher: Arc::new(ApiKeyCipher::from_key([7u8; 32], true)),
    }
}

#[tokio::test]
async fn e2e_provider_update_empty_key_keeps_original() {
    // 回归测试（C6）：更新 Provider 时传空 api_key 或脱敏掩码，必须保留原 key，
    // 覆盖内存与 DB 两侧，避免误把真实 key 覆盖为空串。
    let state = provider_test_state();

    // 1) add provider with key "secret123"
    let mut p = Provider {
        id: String::new(), // add_provider 会自动生成 UUID
        name: "KeyKeeper".into(),
        protocol: ApiProtocol::OpenAIChat,
        base_url: "http://127.0.0.1:1".into(),
        api_key: "secret123".into(),
        models: vec!["mock-gpt".into()],
        model_aliases: Default::default(),
        model_context_lengths: Default::default(),
        enabled: true,
        created_at: 0,
    };
    super::provider::add_provider(&state, p.clone())
        .await
        .expect("add should succeed");
    let id = state.providers.read().await[0].id.clone();

    // 2) update 传空 api_key → 内存与 DB 中 key 均保留为 "secret123"
    p.id = id.clone();
    p.api_key = String::new();
    super::provider::update_provider(&state, &id, p)
        .await
        .expect("update with empty api_key should succeed");
    assert_eq!(
        state.providers.read().await[0].api_key,
        "secret123",
        "empty api_key must not overwrite the stored key"
    );

    // 3) update 传脱敏掩码 → 同样保留原 key
    let p = {
        let stored = state.providers.read().await[0].clone();
        Provider {
            api_key: "••••".to_string(),
            ..stored
        }
    };
    super::provider::update_provider(&state, &id, p)
        .await
        .expect("update with masked api_key should succeed");
    assert_eq!(
        state.providers.read().await[0].api_key,
        "secret123",
        "masked api_key must not overwrite the stored key"
    );

    // 4) 模拟重启：重新从 DB 加载，key 仍为 "secret123"
    let reloaded = {
        let db = state.db.lock().await;
        provider::load_providers_from_db_sync(db.as_ref().unwrap(), &state.api_key_cipher)
    };
    assert_eq!(reloaded.len(), 1);
    assert_eq!(reloaded[0].api_key, "secret123", "DB must keep the original key");
}

#[tokio::test]
async fn e2e_provider_update_missing_id_returns_not_found() {
    // 回归测试（C6）：更新不存在的 id 必须返回 Err（含 "not found"），不再静默成功。
    let state = provider_test_state();

    let p = Provider {
        id: "does-not-exist".into(),
        name: "Ghost".into(),
        protocol: ApiProtocol::OpenAIChat,
        base_url: "http://127.0.0.1:1".into(),
        api_key: "sk-test".into(),
        models: vec!["mock-gpt".into()],
        model_aliases: Default::default(),
        model_context_lengths: Default::default(),
        enabled: true,
        created_at: 0,
    };

    let err = super::provider::update_provider(&state, "does-not-exist", p)
        .await
        .expect_err("updating a non-existent id must fail");
    assert!(
        err.to_lowercase().contains("not found"),
        "error should mention 'not found', got: {:?}",
        err
    );

    // 内存与 DB 均未被改动
    assert!(state.providers.read().await.is_empty());
    let count = {
        let db = state.db.lock().await;
        db.as_ref()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM providers",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    };
    assert_eq!(count, 0, "no row should be inserted for a missing id");
}
