use axum::{response::IntoResponse, routing::post, Json, Router};
use devnexus_lib::api_hub;
use std::sync::Arc;
use std::time::Duration;

async fn mock_chat(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("smoke");
    Json(serde_json::json!({
        "id": "chatcmpl-smoke",
        "object": "chat.completion",
        "created": 1,
        "model": model,
        "choices": [{"index":0,"message":{"role":"assistant","content":"pong-from-mock"},"finish_reason":"stop"}],
        "usage": {"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}
    }))
}

#[tokio::main]
async fn main() {
    let mock = Router::new().route("/v1/chat/completions", post(mock_chat));
    let mock_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = mock_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(mock_listener, mock).await.unwrap();
    });

    let tmp = std::env::temp_dir().join(format!("devnexus-smoke-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let state = api_hub::init(&tmp);
    api_hub::provider::add_provider(
        &state,
        api_hub::types::Provider {
            id: "smoke".into(),
            name: "Smoke Mock".into(),
            protocol: api_hub::types::ApiProtocol::OpenAIChat,
            base_url: format!("http://{}", mock_addr),
            api_key: "x".into(),
            models: vec!["smoke-model".into()],
            model_aliases: Default::default(),
            enabled: true,
            created_at: 0,
        },
    )
    .unwrap();

    let hub_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hub_addr = hub_listener.local_addr().unwrap();
    let app = api_hub::server::build_router(Arc::new(state));
    tokio::spawn(async move {
        axum::serve(hub_listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    println!("HUB_URL=http://{}", hub_addr);

    // self-check with reqwest
    let health: serde_json::Value = reqwest::get(format!("http://{}/health", hub_addr))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    println!("HEALTH={}", health);

    let models: serde_json::Value = reqwest::get(format!("http://{}/v1/models", hub_addr))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    println!("MODELS={}", models);

    let client = reqwest::Client::new();
    let chat: serde_json::Value = client
        .post(format!("http://{}/v1/chat/completions", hub_addr))
        .json(&serde_json::json!({
            "model": "smoke-model",
            "messages": [{"role":"user","content":"ping"}]
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    println!("CHAT={}", chat);

    // keep alive a few seconds for external curl if needed
    if std::env::var("KEEP_ALIVE").is_ok() {
        println!("keeping alive 30s for manual curl...");
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
