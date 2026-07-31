use super::types::{ApiProtocol, AppState, Provider};

/// 路由结果
pub struct RouteResult {
    pub provider: Provider,
    pub model: String,
}

/// 根据模型名找到对应的 Provider（async 版本，使用 tokio RwLock）
pub async fn route_by_model(state: &AppState, model: &str) -> Option<RouteResult> {
    let providers = state.providers.read().await;

    // 1. 精确匹配模型名（忽略大小写，无堆分配）
    for p in providers.iter() {
        if !p.enabled {
            continue;
        }
        if p.models.iter().any(|m| m.eq_ignore_ascii_case(model)) {
            return Some(RouteResult {
                provider: p.clone(),
                model: model.to_string(),
            });
        }
    }

    // 2. 通配符匹配：按协议前缀
    for p in providers.iter() {
        if !p.enabled {
            continue;
        }
        let known_prefixes: &[&str] = match p.protocol {
            ApiProtocol::OpenAIChat | ApiProtocol::OpenAIResponses => {
                &["gpt-", "o1-", "o3-", "o4-", "text-", "dall-e", "tts-", "whisper"]
            }
            ApiProtocol::Anthropic => &["claude-"],
        };

        if known_prefixes.iter().any(|prefix| {
            model.len() >= prefix.len()
                && model.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
        }) {
            return Some(RouteResult {
                provider: p.clone(),
                model: model.to_string(),
            });
        }
    }

    // 3. 不再兜底：未匹配则返回 None，调用方应返回 404
    None
}

/// 拼接 base_url + endpoint，自动去除重叠路径段（防 /v1/v1/ 这种双份）
fn join_path(base: &str, endpoint: &str) -> String {
    let base = base.trim_end_matches('/');
    let endpoint = endpoint.trim_start_matches('/');

    // 如果 endpoint 以 base 最后一段开头，说明重复了
    let base_last = base.rsplit('/').next().unwrap_or("");
    if !base_last.is_empty()
        && endpoint.starts_with(base_last)
        && base != "http"
        && base != "https"
    {
        let rest = &endpoint[base_last.len()..];
        let rest = rest.trim_start_matches('/');
        if rest.is_empty() {
            return base.to_string();
        }
        return format!("{}/{}", base, rest);
    }

    format!("{}/{}", base, endpoint)
}

/// 根据 Provider 协议构建完整的上游 URL
pub fn build_upstream_url(provider: &Provider, endpoint: &str) -> String {
    let base = provider.base_url.trim_end_matches('/');
    join_path(base, endpoint)
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
        assert_eq!(
            join_path("https://api.openai.com", "/v1/responses"),
            "https://api.openai.com/v1/responses"
        );
    }
}
