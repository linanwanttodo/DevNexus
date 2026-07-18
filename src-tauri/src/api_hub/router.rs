use super::types::{ApiProtocol, AppState, Provider};

/// 路由结果
pub struct RouteResult {
    pub provider: Provider,
    pub model: String,
}

/// 根据模型名找到对应的 Provider
pub fn route_by_model(state: &AppState, model: &str) -> Option<RouteResult> {
    let providers = state.providers.read().ok()?;
    let model_lower = model.to_lowercase();

    // 1. 精确匹配模型名
    for p in providers.iter() {
        if !p.enabled {
            continue;
        }
        if p.models.iter().any(|m| m.to_lowercase() == model_lower) {
            return Some(RouteResult {
                provider: p.clone(),
                model: model.to_string(),
            });
        }
    }

    // 2. 通配符匹配：如果模型名包含 provider 的已知模型前缀
    for p in providers.iter() {
        if !p.enabled {
            continue;
        }
        let known_prefixes: &[&str] = match p.protocol {
            ApiProtocol::OpenAIChat | ApiProtocol::OpenAIResponses => {
                &["gpt-", "o1-", "o3-", "text-", "dall-e", "tts-", "whisper"]
            }
            ApiProtocol::Anthropic => &["claude-"],
            ApiProtocol::Gemini => &["gemini-"],
            ApiProtocol::Ollama => &[],
        };

        if known_prefixes.iter().any(|prefix| model_lower.starts_with(prefix)) {
            return Some(RouteResult {
                provider: p.clone(),
                model: model.to_string(),
            });
        }
    }

    // 3. 如果仍不匹配，返回第一个启用的 provider 作为兜底
    for p in providers.iter() {
        if p.enabled {
            return Some(RouteResult {
                provider: p.clone(),
                model: model.to_string(),
            });
        }
    }

    None
}

/// 根据 Provider 协议、模型名和端点模板构建完整的上游 URL
pub fn build_upstream_url(provider: &Provider, endpoint: &str, model: &str) -> String {
    let base = provider.base_url.trim_end_matches('/');
    match provider.protocol {
        ApiProtocol::OpenAIChat | ApiProtocol::OpenAIResponses | ApiProtocol::Anthropic => {
            format!("{}{}", base, endpoint)
        }
        ApiProtocol::Gemini => {
            let model_name = if model.is_empty() {
                provider
                    .models
                    .first()
                    .map(|s| s.as_str())
                    .unwrap_or("gemini-2.0-flash")
            } else {
                model
            };
            if endpoint.contains("{model}") {
                format!("{}{}", base, endpoint.replace("{model}", model_name))
            } else if endpoint.contains("generateContent") {
                format!("{}{}", base, endpoint)
            } else {
                format!("{}/v1/models/{}:generateContent", base, model_name)
            }
        }
        ApiProtocol::Ollama => {
            if endpoint.contains("/chat/completions") || endpoint.contains("/v1/messages") {
                format!("{}/v1/chat/completions", base)
            } else {
                format!("{}{}", base, endpoint)
            }
        }
    }
}
