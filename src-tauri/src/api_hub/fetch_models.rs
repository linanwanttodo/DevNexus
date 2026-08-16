use super::types::{ApiProtocol, FetchedModel};

/// 从 Provider 的模型列表端点获取可用模型
/// `client` 复用 AppState 中已初始化的全局 HTTP client（连接池复用，避免每次新建）
pub async fn fetch_models_from_provider(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    protocol: &ApiProtocol,
) -> Result<Vec<FetchedModel>, String> {
    match protocol {
        ApiProtocol::OpenAIChat | ApiProtocol::OpenAIResponses => {
            fetch_openai_style_models(client, base_url, api_key, false).await
        }
        ApiProtocol::Anthropic => fetch_openai_style_models(client, base_url, api_key, true).await,
        ApiProtocol::Gemini => fetch_gemini_models(client, base_url, api_key).await,
        ApiProtocol::Ollama => fetch_ollama_models(client, base_url).await,
    }
}

/// Ollama 模型列表：GET /api/tags，models[].name 形如 "llama3:8b"（无需认证）
async fn fetch_ollama_models(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<Vec<FetchedModel>, String> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    if !resp.status().is_success() {
        let _error = resp.text().await.unwrap_or_default();
        return Ok(vec![]);
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    let mut models = Vec::new();
    if let Some(arr) = body.get("models").and_then(|m| m.as_array()) {
        for item in arr {
            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let display = item
                .get("details")
                .and_then(|d| d.get("family"))
                .and_then(|f| f.as_str())
                .unwrap_or(name)
                .to_string();
            let size_gb = item.get("size").and_then(|v| v.as_u64()).unwrap_or(0) as f64
                / 1024.0
                / 1024.0
                / 1024.0;
            let mut m = FetchedModel {
                id: name.to_string(),
                name: format!("{display} ({size_gb:.1}GB)"),
                owned_by: Some("ollama".to_string()),
                enabled: true,
            };
            if size_gb <= 0.0 {
                m.name = display;
            }
            models.push(m);
        }
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

/// Gemini 模型列表：GET /v1beta/models，models[].name 形如 "models/gemini-2.5-flash"
async fn fetch_gemini_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<Vec<FetchedModel>, String> {
    let url = format!(
        "{}/v1beta/models?pageSize=100",
        base_url.trim_end_matches('/')
    );
    let mut req = client.get(&url);
    if !api_key.is_empty() {
        req = req.header("x-goog-api-key", api_key);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        let _error = resp.text().await.unwrap_or_default();
        return Ok(vec![]);
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let mut models = Vec::new();
    if let Some(arr) = body.get("models").and_then(|m| m.as_array()) {
        for item in arr {
            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
            // 仅保留支持 generateContent 的对话模型，且去掉 "models/" 前缀
            let methods = item
                .get("supportedGenerationMethods")
                .and_then(|m| m.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .any(|s| s == "generateContent")
                })
                .unwrap_or(true);
            if !methods {
                continue;
            }
            let id = name.strip_prefix("models/").unwrap_or(name);
            if id.is_empty() {
                continue;
            }
            let display = item
                .get("displayName")
                .and_then(|d| d.as_str())
                .unwrap_or(id)
                .to_string();
            models.push(FetchedModel {
                id: id.to_string(),
                name: display,
                owned_by: Some("google".to_string()),
                enabled: true,
            });
        }
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

/// OpenAI 风格的 /v1/models 端点（OpenAI / Anthropic 兼容）
/// `is_anthropic` 决定是否使用 x-api-key + anthropic-version header
async fn fetch_openai_style_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    is_anthropic: bool,
) -> Result<Vec<FetchedModel>, String> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
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
