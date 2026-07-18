use chrono::Utc;
use serde_json::Value;

// ── Request: Responses → Chat Completions ─────────────────────

/// 将 OpenAI /v1/responses 请求转换为 /v1/chat/completions 格式
pub fn responses_to_chat(body: &Value) -> Value {
    let model = body.get("model").and_then(|m| m.as_str()).unwrap_or("");
    let instructions = body.get("instructions").and_then(|i| i.as_str());

    let mut messages: Vec<Value> = Vec::new();

    if let Some(sys) = instructions {
        messages.push(serde_json::json!({
            "role": "system",
            "content": sys
        }));
    }

    if let Some(input) = body.get("input") {
        match input {
            Value::String(s) => {
                messages.push(serde_json::json!({
                    "role": "user",
                    "content": s
                }));
            }
            Value::Array(items) => {
                for item in items {
                    if let Some(text) = item.as_str() {
                        messages.push(serde_json::json!({
                            "role": "user",
                            "content": text
                        }));
                    } else if let Some(tp) = item.get("type").and_then(|t| t.as_str()) {
                        if tp == "message" {
                            let role = item.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                            let content =
                                extract_text_content(item.get("content").unwrap_or(&Value::Null));
                            messages.push(serde_json::json!({
                                "role": role,
                                "content": content
                            }));
                        }
                    } else if let Some(role) = item.get("role").and_then(|r| r.as_str()) {
                        // 兼容直接 {role, content} 数组
                        let content =
                            extract_text_content(item.get("content").unwrap_or(&Value::Null));
                        messages.push(serde_json::json!({
                            "role": role,
                            "content": content
                        }));
                    }
                }
            }
            _ => {}
        }
    }

    if messages.is_empty() {
        messages.push(serde_json::json!({ "role": "user", "content": "" }));
    }

    let mut chat_body = serde_json::json!({
        "model": model,
        "messages": messages,
    });

    if let Some(max_tokens) = body.get("max_output_tokens") {
        chat_body["max_tokens"] = max_tokens.clone();
    }
    if let Some(temp) = body.get("temperature") {
        chat_body["temperature"] = temp.clone();
    }
    if let Some(top_p) = body.get("top_p") {
        chat_body["top_p"] = top_p.clone();
    }
    if let Some(stream) = body.get("stream") {
        chat_body["stream"] = stream.clone();
    }

    chat_body
}

// ── Request: Chat Completions → Responses ─────────────────────

/// 将 /v1/chat/completions **请求** 转换为 /v1/responses **请求**
pub fn chat_request_to_responses(body: &Value) -> Value {
    let model = body.get("model").and_then(|m| m.as_str()).unwrap_or("");
    let mut instructions: Option<String> = None;
    let mut input: Vec<Value> = Vec::new();

    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = extract_text_content(msg.get("content").unwrap_or(&Value::Null));
            match role {
                "system" => {
                    instructions = Some(match instructions {
                        Some(prev) => format!("{}\n\n{}", prev, content),
                        None => content,
                    });
                }
                _ => {
                    input.push(serde_json::json!({
                        "type": "message",
                        "role": role,
                        "content": content
                    }));
                }
            }
        }
    }

    if input.is_empty() {
        input.push(serde_json::json!({
            "type": "message",
            "role": "user",
            "content": ""
        }));
    }

    let mut resp_body = serde_json::json!({
        "model": model,
        "input": input,
    });

    if let Some(sys) = instructions {
        resp_body["instructions"] = Value::String(sys);
    }
    if let Some(max_tokens) = body.get("max_tokens") {
        resp_body["max_output_tokens"] = max_tokens.clone();
    }
    if let Some(temp) = body.get("temperature") {
        resp_body["temperature"] = temp.clone();
    }
    if let Some(top_p) = body.get("top_p") {
        resp_body["top_p"] = top_p.clone();
    }
    if let Some(stream) = body.get("stream") {
        resp_body["stream"] = stream.clone();
    }

    resp_body
}

// ── Response: Chat Completions → Responses ────────────────────

/// 将 /v1/chat/completions 响应转换为 /v1/responses 格式
pub fn chat_to_responses(chat_resp: &Value, _model: &str) -> Value {
    let id = chat_resp.get("id").and_then(|id| id.as_str()).unwrap_or("");
    let model_name = chat_resp
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("");

    let mut content_text = String::new();
    if let Some(choices) = chat_resp.get("choices").and_then(|c| c.as_array()) {
        if let Some(choice) = choices.first() {
            if let Some(message) = choice.get("message") {
                content_text = message
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
            }
        }
    }

    let usage = chat_resp.get("usage").map(|u| {
        serde_json::json!({
            "input_tokens": u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            "output_tokens": u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            "total_tokens": u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        })
    });

    let status = match chat_resp
        .pointer("/choices/0/finish_reason")
        .and_then(|r| r.as_str())
    {
        Some("stop") | Some("length") => "completed",
        _ => "completed",
    };

    serde_json::json!({
        "id": id,
        "object": "response",
        "model": model_name,
        "status": status,
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": content_text}],
            "status": status
        }],
        "usage": usage
    })
}

// ── Response: Responses → Chat Completions ────────────────────

/// 将 /v1/responses 响应转换为 /v1/chat/completions 响应格式
pub fn responses_to_chat_response(resp: &Value, _model: &str) -> Value {
    let id = resp.get("id").and_then(|id| id.as_str()).unwrap_or("");
    let model_name = resp.get("model").and_then(|m| m.as_str()).unwrap_or("");

    let mut content_text = String::new();
    if let Some(output) = resp.get("output").and_then(|o| o.as_array()) {
        for item in output {
            if item.get("type").and_then(|t| t.as_str()) == Some("message") {
                if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
                    for part in content {
                        let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if part_type == "output_text" || part_type == "text" {
                            content_text
                                .push_str(part.get("text").and_then(|t| t.as_str()).unwrap_or(""));
                        }
                    }
                }
            }
        }
    }

    let usage = resp.get("usage").map(|u| {
        let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let output = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        serde_json::json!({
            "prompt_tokens": input,
            "completion_tokens": output,
            "total_tokens": input + output,
        })
    });

    serde_json::json!({
        "id": id,
        "object": "chat.completion",
        "created": Utc::now().timestamp(),
        "model": model_name,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": content_text},
            "finish_reason": "stop"
        }],
        "usage": usage
    })
}

fn extract_text_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(arr) => {
            let mut text = String::new();
            for part in arr {
                if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                    text.push_str(t);
                } else if let Some(t) = part.as_str() {
                    text.push_str(t);
                }
            }
            text
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_responses_to_chat_basic() {
        let body = serde_json::json!({
            "model": "gpt-4o",
            "input": [
                {"type": "message", "role": "user", "content": "Hello"},
                {"type": "message", "role": "assistant", "content": "Hi there"},
                {"type": "message", "role": "user", "content": "How are you?"}
            ],
            "instructions": "You are a helpful assistant."
        });
        let chat = responses_to_chat(&body);
        assert_eq!(chat["model"], "gpt-4o");
        let messages = chat["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "You are a helpful assistant.");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
    }

    #[test]
    fn test_chat_request_to_responses() {
        let body = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "Be brief."},
                {"role": "user", "content": "Hi"}
            ],
            "max_tokens": 100,
            "temperature": 0.2
        });
        let resp_req = chat_request_to_responses(&body);
        assert_eq!(resp_req["model"], "gpt-4o");
        assert_eq!(resp_req["instructions"], "Be brief.");
        assert_eq!(resp_req["max_output_tokens"], 100);
        let input = resp_req["input"].as_array().unwrap();
        assert_eq!(input.len(), 1);
        assert_eq!(input[0]["role"], "user");
    }

    #[test]
    fn test_chat_to_responses_basic() {
        let chat_resp = serde_json::json!({
            "id": "chatcmpl-abc",
            "model": "gpt-4o",
            "choices": [{"message": {"role": "assistant", "content": "Hello!"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });
        let resp = chat_to_responses(&chat_resp, "gpt-4o");
        assert_eq!(resp["object"], "response");
        assert_eq!(resp["id"], "chatcmpl-abc");
        let output = resp["output"].as_array().unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[0]["role"], "assistant");
        assert_eq!(resp["usage"]["input_tokens"], 10);
        assert_eq!(resp["usage"]["output_tokens"], 5);
    }
}
