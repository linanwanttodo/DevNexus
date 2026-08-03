// API Hub 并发基准压测：记录 RPS 与 p50/p95 延迟基线
// 用法: cargo run --example api_hub_bench --manifest-path src-tauri/Cargo.toml
use axum::{response::IntoResponse, routing::post, Json, Router};
use devnexus_lib::api_hub;
use std::sync::Arc;
use std::time::Duration;

async fn mock_chat(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("bench");
    Json(serde_json::json!({
        "id": "chatcmpl-bench",
        "object": "chat.completion",
        "created": 1,
        "model": model,
        "choices": [{"index":0,"message":{"role":"assistant","content":"pong-from-mock"},"finish_reason":"stop"}],
        "usage": {"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}
    }))
}

#[tokio::main]
async fn main() {
    let concurrency: usize = std::env::var("BENCH_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);
    let total_requests: usize = std::env::var("BENCH_TOTAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);

    // 1) mock 上游
    let mock = Router::new().route("/v1/chat/completions", post(mock_chat));
    let mock_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = mock_listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(mock_listener, mock).await.unwrap() });

    // 2) hub
    let tmp = std::env::temp_dir().join(format!("devnexus-bench-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let state = api_hub::init(&tmp);
    api_hub::provider::add_provider(
        &state,
        api_hub::types::Provider {
            id: "bench".into(),
            name: "Bench Mock".into(),
            protocol: api_hub::types::ApiProtocol::OpenAIChat,
            base_url: format!("http://{}", mock_addr),
            api_key: "x".into(),
            models: vec!["bench-model".into()],
            model_aliases: Default::default(),
            model_context_lengths: Default::default(),
            enabled: true,
            created_at: 0,
        },
    )
    .await
    .unwrap();

    let token = state.auth_token.clone();
    let hub_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hub_addr = hub_listener.local_addr().unwrap();
    let app = api_hub::server::build_router(Arc::new(state));
    tokio::spawn(async move { axum::serve(hub_listener, app).await.unwrap() });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let url = format!("http://{}/v1/chat/completions", hub_addr);
    let body = serde_json::json!({
        "model": "bench-model",
        "messages": [{"role":"user","content":"ping"}]
    });

    // 3) 预热 10 请求
    for _ in 0..10 {
        let _ = client
            .post(&url)
            .header("X-DevNexus-Token", &token)
            .json(&body)
            .send()
            .await
            .unwrap();
    }

    // 4) 并发压测：信号量限制并发，记录每次延迟
    let sem = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let start = std::time::Instant::now();
    let mut latencies: Vec<Duration> = Vec::with_capacity(total_requests);
    let mut failed = 0usize;

    let mut handles = Vec::new();
    for _ in 0..total_requests {
        let sem = sem.clone();
        let client = client.clone();
        let url = url.clone();
        let body = body.clone();
        let token = token.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let t0 = std::time::Instant::now();
            let resp = client
                .post(&url)
                .header("X-DevNexus-Token", &token)
                .json(&body)
                .send()
                .await;
            let elapsed = t0.elapsed();
            match resp {
                Ok(r) if r.status().is_success() => Ok(elapsed),
                _ => Err(elapsed),
            }
        }));
    }

    for h in handles {
        match h.await.unwrap() {
            Ok(e) => latencies.push(e),
            Err(_) => failed += 1,
        }
    }
    let total_elapsed = start.elapsed();

    // 5) 统计 p50/p95
    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[(latencies.len() as f64 * 0.95) as usize];
    let rps = total_requests as f64 / total_elapsed.as_secs_f64();

    println!("=== API Hub 基准基线 ===");
    println!(
        "并发: {} | 总请求: {} | 失败: {}",
        concurrency, total_requests, failed
    );
    println!(
        "总耗时: {:.2}s | RPS: {:.0}",
        total_elapsed.as_secs_f64(),
        rps
    );
    println!("p50: {:?} | p95: {:?}", p50, p95);
    println!(
        "环境: {} | {:?}",
        std::env::consts::OS,
        std::thread::available_parallelism().map(|n| n.get())
    );
}
