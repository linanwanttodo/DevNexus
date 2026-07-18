use crate::api_hub::types::{
    AnthropicContent, AnthropicMessage, AnthropicRequest, AnthropicResponse, AnthropicUsage,
    ChatMessage, OpenAIChatRequest, OpenAIChatResponse, OpenAIChoice, OpenAIUsage,
};
use chrono::Utc;
use serde_json::Value;

// ── Request: OpenAI Chat → Anthropic ──────────────────────────

/// 将 OpenAI ChatCompletion 请求转换为 Anthropic Messages 请求
pub fn openai_to_anthropic(req: &OpenAIChatRequest) -> AnthropicRequest {
    let mut system_parts: Vec<String> = Vec::new();
    let mut messages: Vec<AnthropicMessage> = Vec::new();

    for msg in &req.messages {
        let role = msg.role.as_str();
        let content = extract_text_content(&msg.content);

        match role {
            "system" => system_parts.push(content),
            "assistant" => messages.push(AnthropicMessage {
                role: "assistant".to_string(),
                content,
            }),
            _ => messages.push(AnthropicMessage {
                role: "user".to_string(),
                content,
            }),
        }
    }

    if messages.is_empty() {
        messages.push(AnthropicMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        });
    }

    let system = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };

    AnthropicRequest {
        model: req.model.clone(),
        messages,
        system,
        max_tokens: req.max_tokens.unwrap_or(4096),
        temperature: req.temperature,
        stream: req.stream,
        stop_sequences: req.stop.clone(),
    }
}

// ── Request: Anthropic → OpenAI Chat ──────────────────────────

/// 将 Anthropic 请求转换为 OpenAI 格式
pub fn anthropic_to_openai_req(req: &AnthropicRequest) -> OpenAIChatRequest {
    let mut messages = Vec::new();

    if let Some(ref sys) = req.system {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: Value::String(sys.clone()),
        });
    }

    for msg in &req.messages {
        messages.push(ChatMessage {
            role: msg.role.clone(),
            content: Value::String(msg.content.clone()),
        });
    }

    OpenAIChatRequest {
        model: req.model.clone(),
        messages,
        temperature: req.temperature,
        max_tokens: Some(req.max_tokens),
        stream: req.stream,
        stop: req.stop_sequences.clone(),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
    }
}

// ── Response: Anthropic → OpenAI Chat ─────────────────────────

/// 将 Anthropic 响应转换为 OpenAI ChatCompletion 格式
pub fn anthropic_to_openai(
    anthropic_id: &str,
    model: &str,
    resp: &AnthropicResponse,
) -> OpenAIChatResponse {
    let mut content = String::new();
    for block in &resp.content {
        if block.content_type == "text" {
            content.push_str(&block.text);
        }
    }

    let input_tokens = resp.usage.as_ref().map(|u| u.input_tokens).unwrap_or(0);
    let output_tokens = resp.usage.as_ref().map(|u| u.output_tokens).unwrap_or(0);

    OpenAIChatResponse {
        id: anthropic_id.to_string(),
        object: "chat.completion".to_string(),
        created: Utc::now().timestamp(),
        model: model.to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: Value::String(content),
            },
            finish_reason: Some(map_anthropic_stop_reason(resp.stop_reason.as_deref())),
        }],
        usage: Some(OpenAIUsage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
        }),
    }
}

// ── Response: OpenAI Chat → Anthropic ─────────────────────────

/// 将 OpenAI ChatCompletion JSON 响应转换为 Anthropic Messages 格式
pub fn openai_response_to_anthropic(resp: &Value, model: &str) -> Value {
    let text = resp
        .pointer("/choices/0/message/content")
        .and_then(|t| match t {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        })
        .unwrap_or("");
    let input_tokens = resp
        .pointer("/usage/prompt_tokens")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);
    let output_tokens = resp
        .pointer("/usage/completion_tokens")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);
    let stop_reason = match resp
        .pointer("/choices/0/finish_reason")
        .and_then(|r| r.as_str())
    {
        Some("length") => "max_tokens",
        Some("tool_calls") => "tool_use",
        _ => "end_turn",
    };

    serde_json::json!({
        "id": format!("msg_{}", uuid::Uuid::new_v4()),
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": text}],
        "model": model,
        "stop_reason": stop_reason,
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens
        }
    })
}

/// 从 Value 构建 AnthropicResponse（用于类型化路径）
#[allow(dead_code)]
pub fn openai_to_anthropic_response(resp: &OpenAIChatResponse) -> AnthropicResponse {
    let text = resp
        .choices
        .first()
        .map(|c| extract_text_content(&c.message.content))
        .unwrap_or_default();
    let input_tokens = resp.usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0);
    let output_tokens = resp
        .usage
        .as_ref()
        .map(|u| u.completion_tokens)
        .unwrap_or(0);
    let stop_reason = resp
        .choices
        .first()
        .and_then(|c| c.finish_reason.as_deref())
        .map(|r| match r {
            "length" => "max_tokens".to_string(),
            "tool_calls" => "tool_use".to_string(),
            _ => "end_turn".to_string(),
        });

    AnthropicResponse {
        id: format!("msg_{}", uuid::Uuid::new_v4()),
        model: resp.model.clone(),
        response_type: "message".to_string(),
        content: vec![AnthropicContent {
            content_type: "text".to_string(),
            text,
        }],
        usage: Some(AnthropicUsage {
            input_tokens,
            output_tokens,
        }),
        stop_reason,
    }
}

// ── Helpers ───────────────────────────────────────────────────

fn extract_text_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => {
            let mut text = String::new();
            for block in blocks {
                if let Some(block_type) = block.get("type").and_then(|t| t.as_str()) {
                    match block_type {
                        "text" => {
                            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                                if !text.is_empty() {
                                    text.push('\n');
                                }
                                text.push_str(t);
                            }
                        }
                        "image_url" => {
                            text.push_str("[Image: ");
                            if let Some(url) =
                                block.pointer("/image_url/url").and_then(|u| u.as_str())
                            {
                                text.push_str(url);
                            }
                            text.push(']');
                        }
                        _ => {
                            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                                text.push_str(t);
                            }
                        }
                    }
                }
            }
            text
        }
        _ => String::new(),
    }
}

fn map_anthropic_stop_reason(reason: Option<&str>) -> String {
    match reason {
        Some("end_turn") => "stop".to_string(),
        Some("max_tokens") => "length".to_string(),
        Some("stop_sequence") => "stop".to_string(),
        Some("tool_use") => "tool_calls".to_string(),
        _ => "stop".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_message_extraction() {
        let req = OpenAIChatRequest {
            model: "gpt-4o".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Value::String("You are a helpful assistant.".to_string()),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Value::String("Hello".to_string()),
                },
            ],
            temperature: Some(0.7),
            max_tokens: Some(1024),
            stream: None,
            stop: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
        };
        let result = openai_to_anthropic(&req);
        assert_eq!(
            result.system,
            Some("You are a helpful assistant.".to_string())
        );
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, "user");
    }

    #[test]
    fn test_openai_response_to_anthropic() {
        let resp = serde_json::json!({
            "id": "chatcmpl-1",
            "choices": [{"message": {"role": "assistant", "content": "Hi"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 2, "total_tokens": 7}
        });
        let anth = openai_response_to_anthropic(&resp, "claude-3");
        assert_eq!(anth["type"], "message");
        assert_eq!(anth["content"][0]["text"], "Hi");
        assert_eq!(anth["usage"]["input_tokens"], 5);
        assert_eq!(anth["stop_reason"], "end_turn");
    }
}
