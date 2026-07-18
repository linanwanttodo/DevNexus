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

        if known_prefixes
            .iter()
            .any(|prefix| model_lower.starts_with(prefix))
        {
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

/// 拼接 base_url + endpoint，自动去除重叠路径段（防 /v1/v1/ 这种双份）
fn join_path(base: &str, endpoint: &str) -> String {
    let base = base.trim_end_matches('/');
    let endpoint = endpoint.trim_start_matches('/');

    // 如果 endpoint 以 base 最后一段开头，说明重复了：取 base + endpoint - overlap
    // 例: base=/api/v1, endpoint=v1/chat → /api/v1/chat
    let base_last = base.rsplit('/').next().unwrap_or("");
    if !base_last.is_empty() && endpoint.starts_with(base_last) && base != "http" && base != "https"
    {
        // 跳过 endpoint 中的重叠前缀
        let rest = &endpoint[base_last.len()..]; // "v1/chat" → "/chat"
        let rest = rest.trim_start_matches('/');
        if rest.is_empty() {
            return base.to_string();
        }
        return format!("{}/{}", base, rest);
    }

    format!("{}/{}", base, endpoint)
}

/// 根据 Provider 协议、模型名和端点模板构建完整的上游 URL
pub fn build_upstream_url(provider: &Provider, endpoint: &str, model: &str) -> String {
    let base = provider.base_url.trim_end_matches('/');
    match provider.protocol {
        ApiProtocol::OpenAIChat | ApiProtocol::OpenAIResponses | ApiProtocol::Anthropic => {
            join_path(base, endpoint)
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
                join_path(base, &endpoint.replace("{model}", model_name))
            } else if endpoint.contains("generateContent") {
                join_path(base, endpoint)
            } else {
                join_path(base, &format!("/v1/models/{}:generateContent", model_name))
            }
        }
        ApiProtocol::Ollama => {
            if endpoint.contains("/chat/completions") || endpoint.contains("/v1/messages") {
                join_path(base, "/v1/chat/completions")
            } else {
                join_path(base, endpoint)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::join_path;

    #[test]
    fn test_join_path_normal() {
        assert_eq!(
            join_path("https://api.openai.com", "/v1/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_join_path_double_v1() {
        // base 已经有 /v1，endpoint 也有 /v1 → 消重
        assert_eq!(
            join_path("https://gy.hetaosu.xyz/v1", "/v1/chat/completions"),
            "https://gy.hetaosu.xyz/v1/chat/completions"
        );
    }

    #[test]
    fn test_join_path_trailing_slash() {
        assert_eq!(
            join_path("https://example.com/api/", "/api/method"),
            "https://example.com/api/method"
        );
    }

    #[test]
    fn test_join_path_no_overlap() {
        assert_eq!(
            join_path("https://example.com", "/api/v1/method"),
            "https://example.com/api/v1/method"
        );
    }

    #[test]
    fn test_join_path_base_has_no_v1() {
        // base 无 /v1，endpoint 有 /v1 → 正常拼接
        assert_eq!(
            join_path("https://api.openai.com", "/v1/responses"),
            "https://api.openai.com/v1/responses"
        );
    }
}
