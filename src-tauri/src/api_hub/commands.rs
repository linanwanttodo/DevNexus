use super::types::AppState;
use super::types::FetchedModel;
use super::types::Provider;
use std::sync::atomic::Ordering;
use tauri::State;

// ── Provider Management ───────────────────────────────────────

/// 对 api_key 脱敏：保留首尾各 4 位，中间用掩码替代（前端不接触明文 key）
fn mask_api_key(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    if key.len() > 12 && key.is_ascii() {
        format!("{}••••{}", &key[..4], &key[key.len() - 4..])
    } else {
        "••••".to_string()
    }
}

#[tauri::command]
pub async fn api_hub_list_providers(state: State<'_, AppState>) -> Result<Vec<Provider>, String> {
    let providers = state.providers.read().await;
    Ok(providers
        .iter()
        .map(|p| Provider {
            api_key: mask_api_key(&p.api_key),
            ..p.clone()
        })
        .collect())
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
    super::ensure_started(state.inner());
    let token = &state.auth_token;
    let masked = if token.len() > 12 && token.is_ascii() {
        format!("{}••••{}", &token[..4], &token[token.len() - 4..])
    } else if token.is_empty() {
        String::new()
    } else {
        "••••".to_string()
    };
    serde_json::json!({
        "running": state.running.load(Ordering::SeqCst),
        "port": 3456,
        "version": env!("CARGO_PKG_VERSION"),
        "auth_token": masked,
        "auth_token_masked": masked,
        "key_encrypted": state.api_key_cipher.is_encrypted()
    })
}

#[tauri::command]
pub fn api_hub_get_token(state: State<'_, AppState>) -> String {
    // 单独获取明文 token，需前端在已认证上下文调用（Endpoints 页按需获取）
    state.auth_token.clone()
}

// ── Fetch Models from Provider API ────────────────────────────

#[tauri::command]
pub async fn api_hub_fetch_models(
    state: State<'_, AppState>,
    base_url: String,
    api_key: String,
    protocol: String,
    provider_id: Option<String>,
) -> Result<Vec<FetchedModel>, String> {
    let pt = super::types::ApiProtocol::from_protocol_str(&protocol).ok_or_else(|| {
        format!(
            "Unknown protocol: '{}'. Supported: openai_chat, openai_responses, anthropic, gemini, ollama",
            protocol
        )
    })?;

    // 编辑已有 Provider 时前端持有的是脱敏 key，此处解析回已存储的真实 key
    let api_key = if api_key.contains("••••") {
        let providers = state.providers.read().await;
        provider_id
            .as_deref()
            .and_then(|id| providers.iter().find(|p| p.id == id))
            .map(|p| p.api_key.clone())
            .unwrap_or_default()
    } else {
        api_key
    };

    super::fetch_models::fetch_models_from_provider(&state.http_client, &base_url, &api_key, &pt)
        .await
}
