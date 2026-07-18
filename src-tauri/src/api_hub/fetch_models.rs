use super::types::{ApiProtocol, FetchedModel};

/// 从 Provider 的 /v1/models 端点获取可用模型列表
pub async fn fetch_models_from_provider(
    base_url: &str,
    api_key: &str,
    protocol: &ApiProtocol,
) -> Result<Vec<FetchedModel>, String> {
    match protocol {
        ApiProtocol::Ollama => fetch_ollama_models(base_url).await,
        ApiProtocol::Gemini => {
            // Gemini 没有标准 /v1/models 端点，返回预设列表
            Ok(predefined_gemini_models())
        }
        ApiProtocol::OpenAIChat | ApiProtocol::OpenAIResponses | ApiProtocol::Anthropic => {
            fetch_openai_style_models(base_url, api_key).await
        }
    }
}

/// OpenAI 风格的 /v1/models 端点（OpenAI / Anthropic 兼容）
async fn fetch_openai_style_models(
    base_url: &str,
    api_key: &str,
) -> Result<Vec<FetchedModel>, String> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create client: {}", e))?;

    let mut req = client.get(&url);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }
    // Anthropic 兼容
    if !api_key.is_empty() && base_url.contains("anthropic") {
        req = req
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01");
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

/// Ollama 的 /api/tags 端点
async fn fetch_ollama_models(base_url: &str) -> Result<Vec<FetchedModel>, String> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create client: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let mut models = Vec::new();
    if let Some(models_arr) = body.get("models").and_then(|m| m.as_array()) {
        for m in models_arr {
            let name = m
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("unknown")
                .to_string();
            models.push(FetchedModel {
                id: name.clone(),
                name,
                owned_by: Some("Ollama".to_string()),
                enabled: true,
            });
        }
    }

    Ok(models)
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

/// 预设的 Gemini 模型列表
fn predefined_gemini_models() -> Vec<FetchedModel> {
    vec![
        FetchedModel {
            id: "gemini-2.0-flash".to_string(),
            name: "Gemini 2.0 Flash".to_string(),
            owned_by: Some("Google".to_string()),
            enabled: true,
        },
        FetchedModel {
            id: "gemini-2.0-flash-lite".to_string(),
            name: "Gemini 2.0 Flash Lite".to_string(),
            owned_by: Some("Google".to_string()),
            enabled: true,
        },
        FetchedModel {
            id: "gemini-1.5-pro".to_string(),
            name: "Gemini 1.5 Pro".to_string(),
            owned_by: Some("Google".to_string()),
            enabled: true,
        },
        FetchedModel {
            id: "gemini-1.5-flash".to_string(),
            name: "Gemini 1.5 Flash".to_string(),
            owned_by: Some("Google".to_string()),
            enabled: true,
        },
        FetchedModel {
            id: "gemini-2.5-pro-preview-06-05".to_string(),
            name: "Gemini 2.5 Pro Preview".to_string(),
            owned_by: Some("Google".to_string()),
            enabled: true,
        },
    ]
}
