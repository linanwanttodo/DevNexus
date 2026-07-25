use super::types::{ApiProtocol, FetchedModel};

/// 从 Provider 的模型列表端点获取可用模型
pub async fn fetch_models_from_provider(
    base_url: &str,
    api_key: &str,
    protocol: &ApiProtocol,
) -> Result<Vec<FetchedModel>, String> {
    match protocol {
        ApiProtocol::OpenAIChat | ApiProtocol::OpenAIResponses => {
            fetch_openai_style_models(base_url, api_key, false).await
        }
        ApiProtocol::Anthropic => fetch_openai_style_models(base_url, api_key, true).await,
    }
}

/// OpenAI 风格的 /v1/models 端点（OpenAI / Anthropic 兼容）
/// `is_anthropic` 决定是否使用 x-api-key + anthropic-version header
async fn fetch_openai_style_models(
    base_url: &str,
    api_key: &str,
    is_anthropic: bool,
) -> Result<Vec<FetchedModel>, String> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create client: {}", e))?;

    let mut req = client.get(&url);
    if !api_key.is_empty() {
        if is_anthropic {
            req = req
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01");
        } else {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    let status = resp.status();

    if status.is_success() {
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let models = parse_models_response(&body);
        Ok(models)
    } else {
        // 如果 /v1/models 不可用，返回空列表
        let _error = resp.text().await.unwrap_or_default();
        Ok(vec![])
    }
}

/// 解析 OpenAI /v1/models 响应
fn parse_models_response(body: &serde_json::Value) -> Vec<FetchedModel> {
    let mut models = Vec::new();

    if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
        for item in data {
            let id = item
                .get("id")
                .and_then(|id| id.as_str())
                .unwrap_or("unknown")
                .to_string();
            let owned_by = item
                .get("owned_by")
                .and_then(|o| o.as_str())
                .map(|s| s.to_string());

            models.push(FetchedModel {
                id: id.clone(),
                name: id,
                owned_by,
                enabled: true,
            });
        }
    }

    // 按 ID 排序
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models
}
