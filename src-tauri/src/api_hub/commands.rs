use super::types::AppState;
use super::types::FetchedModel;
use super::types::Provider;
use std::sync::atomic::Ordering;
use tauri::State;

// ── Provider Management ───────────────────────────────────────

#[tauri::command]
pub async fn api_hub_list_providers(state: State<'_, AppState>) -> Result<Vec<Provider>, String> {
    let providers = state.providers.read().await;
    Ok(providers.clone())
}

#[tauri::command]
pub async fn api_hub_add_provider(
    state: State<'_, AppState>,
    provider: Provider,
) -> Result<(), String> {
    super::provider::add_provider(state.inner(), provider).await
}

#[tauri::command]
pub async fn api_hub_delete_provider(state: State<'_, AppState>, id: String) -> Result<(), String> {
    super::provider::delete_provider(state.inner(), &id).await
}

#[tauri::command]
pub async fn api_hub_update_provider(
    state: State<'_, AppState>,
    id: String,
    provider: Provider,
) -> Result<(), String> {
    super::provider::update_provider(state.inner(), &id, provider).await
}

// ── Usage & Logs ──────────────────────────────────────────────

#[tauri::command]
pub async fn api_hub_get_logs(
    state: State<'_, AppState>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<super::types::RequestLog>, String> {
    Ok(super::usage::get_logs(state.inner(), limit.unwrap_or(50), offset.unwrap_or(0)).await)
}

#[tauri::command]
pub async fn api_hub_get_usage_stats(
    state: State<'_, AppState>,
) -> Result<super::usage::UsageStats, String> {
    Ok(super::usage::get_usage_stats(state.inner()).await)
}

// ── Server Status ─────────────────────────────────────────────

#[tauri::command]
pub fn api_hub_status(state: State<'_, AppState>) -> serde_json::Value {
    serde_json::json!({
        "running": state.running.load(Ordering::SeqCst),
        "port": 3456,
        "version": env!("CARGO_PKG_VERSION")
    })
}

// ── Fetch Models from Provider API ────────────────────────────

#[tauri::command]
pub async fn api_hub_fetch_models(
    base_url: String,
    api_key: String,
    protocol: String,
) -> Result<Vec<FetchedModel>, String> {
    let pt = super::types::ApiProtocol::from_protocol_str(&protocol)
        .ok_or_else(|| format!("Unknown protocol: '{}'. Supported: openai_chat, openai_responses, anthropic", protocol))?;
    super::fetch_models::fetch_models_from_provider(&base_url, &api_key, &pt).await
}
