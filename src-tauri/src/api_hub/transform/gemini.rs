use chrono::Utc;
use serde_json::Value;

// ── Request: OpenAI Chat → Gemini generateContent ─────────────

/// 将 OpenAI ChatCompletion 请求转换为 Gemini generateContent 请求
pub fn openai_to_gemini(body: &Value) -> Value {
    let mut contents: Vec<Value> = Vec::new();
    let mut system_parts: Vec<String> = Vec::new();

    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let text = extract_text_content(msg.get("content").unwrap_or(&Value::Null));

            match role {
                "system" => {
                    if !text.is_empty() {
                        system_parts.push(text);
                    }
                }
                "assistant" => {
                    contents.push(serde_json::json!({
                        "role": "model",
                        "parts": [{"text": text}]
                    }));
                }
                _ => {
                    contents.push(serde_json::json!({
                        "role": "user",
                        "parts": [{"text": text}]
                    }));
                }
            }
        }
    }

    // Gemini 要求至少一条 content
    if contents.is_empty() {
        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{"text": "Hello"}]
        }));
    }

    let mut generation_config = serde_json::Map::new();
    if let Some(temp) = body.get("temperature") {
        generation_config.insert("temperature".to_string(), temp.clone());
    }
    if let Some(max_tokens) = body.get("max_tokens") {
        generation_config.insert("maxOutputTokens".to_string(), max_tokens.clone());
    }
    if let Some(top_p) = body.get("top_p") {
        generation_config.insert("topP".to_string(), top_p.clone());
    }
    if let Some(stop) = body.get("stop") {
        generation_config.insert("stopSequences".to_string(), stop.clone());
    }

    let mut gemini_body = serde_json::json!({
        "contents": contents,
        "generationConfig": Value::Object(generation_config),
    });

    if !system_parts.is_empty() {
        gemini_body["systemInstruction"] = serde_json::json!({
            "parts": [{"text": system_parts.join("\n\n")}]
        });
    }

    gemini_body
}

// ── Response: Gemini → OpenAI Chat ────────────────────────────

/// 将 Gemini generateContent 响应转换为 OpenAI ChatCompletion 格式
pub fn gemini_to_openai(resp: &Value, model: &str) -> Value {
    let text = extract_gemini_text(resp);
    let finish_reason = map_gemini_finish_reason(
        resp.pointer("/candidates/0/finishReason")
            .and_then(|r| r.as_str()),
    );

    let (prompt_tokens, completion_tokens) = extract_gemini_usage(resp);

    serde_json::json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": text},
            "finish_reason": finish_reason
        }],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + completion_tokens
        }
    })
}

// ── Helpers ───────────────────────────────────────────────────

fn extract_text_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(arr) => {
            let mut text = String::new();
            for part in arr {
                if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(t);
                } else if let Some(t) = part.as_str() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(t);
                }
            }
            text
        }
        _ => String::new(),
    }
}

fn extract_gemini_text(resp: &Value) -> String {
    if let Some(parts) = resp
        .pointer("/candidates/0/content/parts")
        .and_then(|p| p.as_array())
    {
        let mut text = String::new();
        for part in parts {
            if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                text.push_str(t);
            }
        }
        return text;
    }
    resp.pointer("/candidates/0/content/parts/0/text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string()
}

fn extract_gemini_usage(resp: &Value) -> (u32, u32) {
    let usage = resp.get("usageMetadata");
    let prompt = usage
        .and_then(|u| u.get("promptTokenCount"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0) as u32;
    let completion = usage
        .and_then(|u| {
            u.get("candidatesTokenCount")
                .or_else(|| u.get("completionTokenCount"))
        })
        .and_then(|t| t.as_u64())
        .unwrap_or(0) as u32;
    (prompt, completion)
}

fn map_gemini_finish_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("MAX_TOKENS") => "length",
        Some("SAFETY") | Some("RECITATION") | Some("BLOCKLIST") => "content_filter",
        Some("STOP") | Some("END_TURN") => "stop",
        _ => "stop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_to_gemini_with_system() {
        let body = serde_json::json!({
            "model": "gemini-2.0-flash",
            "messages": [
                {"role": "system", "content": "Be concise."},
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi"},
                {"role": "user", "content": "How are you?"}
            ],
            "temperature": 0.5,
            "max_tokens": 256
        });
        let gemini = openai_to_gemini(&body);
        assert!(gemini.get("systemInstruction").is_some());
        let contents = gemini["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(gemini["generationConfig"]["temperature"], 0.5);
        assert_eq!(gemini["generationConfig"]["maxOutputTokens"], 256);
    }

    #[test]
    fn test_gemini_to_openai() {
        let resp = serde_json::json!({
            "candidates": [{
                "content": {"parts": [{"text": "Hello from Gemini"}], "role": "model"},
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5
            }
        });
        let oai = gemini_to_openai(&resp, "gemini-2.0-flash");
        assert_eq!(oai["object"], "chat.completion");
        assert_eq!(oai["choices"][0]["message"]["content"], "Hello from Gemini");
        assert_eq!(oai["usage"]["prompt_tokens"], 10);
        assert_eq!(oai["usage"]["completion_tokens"], 5);
    }
}
