//! Google Gemini（generativelanguage.googleapis.com）协议转换：
//! 内部 OpenAIChat 格式 ↔ Gemini generateContent 格式。
//!
//! Gemini 要点：
//! - 端点路径含模型名：/v1beta/models/{model}:generateContent（流式 :streamGenerateContent?alt=sse）
//! - 认证：x-goog-api-key 头
//! - 消息：contents[].role = user|model，文本在 parts[].text；system 消息 → systemInstruction
//! - 采样参数：generationConfig.{temperature,topP,maxOutputTokens,stopSequences}
//! - 用量：usageMetadata.{promptTokenCount,candidatesTokenCount,totalTokenCount}

use serde_json::{json, Value};

use super::super::types::OpenAIChatRequest;

/// OpenAIChat 请求 → Gemini generateContent 请求体
pub fn openai_to_gemini(req: &OpenAIChatRequest) -> Value {
    let mut system_parts: Vec<Value> = Vec::new();
    let mut contents: Vec<Value> = Vec::new();
    for m in &req.messages {
        let text = message_text(&m.content);
        match m.role.as_str() {
            "system" => system_parts.push(json!({ "text": text })),
            "assistant" => contents.push(json!({ "role": "model", "parts": [{ "text": text }] })),
            _ => contents.push(json!({ "role": "user", "parts": [{ "text": text }] })),
        }
    }

    let mut generation_config = serde_json::Map::new();
    if let Some(t) = req.temperature {
        generation_config.insert("temperature".into(), json!(t));
    }
    if let Some(t) = req.top_p {
        generation_config.insert("topP".into(), json!(t));
    }
    if let Some(m) = req.max_tokens {
        generation_config.insert("maxOutputTokens".into(), json!(m));
    }
    if let Some(s) = &req.stop {
        generation_config.insert("stopSequences".into(), json!(s));
    }

    let mut out = json!({ "contents": contents });
    if !system_parts.is_empty() {
        out["systemInstruction"] = json!({ "parts": system_parts });
    }
    if !generation_config.is_empty() {
        out["generationConfig"] = Value::Object(generation_config);
    }
    out
}

/// content 字段（String 或多模态块数组）拼接为纯文本
fn message_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn candidates_text(resp: &Value) -> String {
    resp.pointer("/candidates/0/content/parts")
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn finish_reason(gemini_reason: Option<&str>) -> Option<&'static str> {
    match gemini_reason {
        Some("STOP") | None => Some("stop"),
        Some("MAX_TOKENS") => Some("length"),
        Some("SAFETY") | Some("RECITATION") | Some("BLOCKLIST") => Some("content_filter"),
        Some(_) => Some("stop"),
    }
}

fn usage_json(resp: &Value) -> Value {
    json!({
        "prompt_tokens": resp.pointer("/usageMetadata/promptTokenCount").and_then(|v| v.as_u64()).unwrap_or(0),
        "completion_tokens": resp.pointer("/usageMetadata/candidatesTokenCount").and_then(|v| v.as_u64()).unwrap_or(0),
        "total_tokens": resp.pointer("/usageMetadata/totalTokenCount").and_then(|v| v.as_u64()).unwrap_or(0),
    })
}

/// Gemini generateContent 响应 → OpenAIChat 响应
pub fn gemini_to_openai(id: &str, model: &str, resp: &Value) -> Value {
    let finish = finish_reason(
        resp.pointer("/candidates/0/finishReason")
            .and_then(|f| f.as_str()),
    )
    .unwrap_or("stop");
    json!({
        "id": id,
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": candidates_text(resp) },
            "finish_reason": finish,
        }],
        "usage": usage_json(resp),
    })
}

/// Gemini 流式 data 载荷 → OpenAI chat.completion.chunk 行（含末尾 [DONE]）。
/// Gemini chunk 不携带模型名/响应 id，由调用方传入；首个内容块补 role。
/// `started` 标记是否已发过首块（由 StreamState 维护）。
pub fn gemini_chunk_to_openai_lines(
    data: &Value,
    model: &str,
    id: &str,
    started: &mut bool,
) -> Vec<String> {
    let text = candidates_text(data);
    let finish = data
        .pointer("/candidates/0/finishReason")
        .and_then(|f| f.as_str())
        .and_then(|r| finish_reason(Some(r)));
    // usage_json 期望完整响应（内部 pointer 到 /usageMetadata/…），此处 data 即完整 chunk
    let usage = if data.get("usageMetadata").is_some() {
        Some(usage_json(data))
    } else {
        None
    };

    let mut out: Vec<String> = Vec::new();
    let has_delta = !text.is_empty() || finish.is_some();
    if has_delta {
        let mut delta = serde_json::Map::new();
        if !*started {
            delta.insert("role".into(), json!("assistant"));
            if !text.is_empty() {
                delta.insert("content".into(), json!(text));
            }
            *started = true;
        } else if !text.is_empty() {
            delta.insert("content".into(), json!(text));
        }
        let chunk = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": chrono::Utc::now().timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "delta": if delta.is_empty() { json!({}) } else { Value::Object(delta) },
                "finish_reason": finish,
            }],
        });
        out.push(format!("data: {chunk}"));
        out.push(String::new());
    }

    if let Some(u) = usage {
        // 独立 usage chunk（choices 为空数组），对齐 stream_options.include_usage 语义
        let chunk = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": chrono::Utc::now().timestamp(),
            "model": model,
            "choices": [],
            "usage": u,
        });
        out.push(format!("data: {chunk}"));
        out.push(String::new());
    }

    // Gemini 流自然结束（无显式终止标记），在带 finishReason 的块后补 [DONE]
    if finish.is_some() {
        out.push("data: [DONE]".to_string());
        out.push(String::new());
    }
    out
}

/// Gemini generateContent 请求 → 内部 OpenAIChat 请求（客户端方向）：
/// contents → messages，systemInstruction → system 消息，generationConfig → 采样参数
pub fn gemini_to_openai_req(body: &Value) -> Result<OpenAIChatRequest, String> {
    let contents = body
        .get("contents")
        .and_then(|c| c.as_array())
        .ok_or("Gemini request missing 'contents' array")?;
    if contents.is_empty() {
        return Err("Gemini request has empty 'contents'".to_string());
    }

    let mut messages: Vec<Value> = Vec::new();
    if let Some(sys) = body
        .pointer("/systemInstruction/parts")
        .and_then(|p| p.as_array())
    {
        let text: String = sys
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("");
        if !text.is_empty() {
            messages.push(json!({ "role": "system", "content": text }));
        }
    }
    for c in contents {
        let role = match c.get("role").and_then(|r| r.as_str()) {
            Some("model") => "assistant",
            _ => "user",
        };
        let text = c
            .get("parts")
            .and_then(|p| p.as_array())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();
        messages.push(json!({ "role": role, "content": text }));
    }

    let cfg = body.get("generationConfig");
    let num = |key: &str| -> Option<f32> {
        cfg.and_then(|o| o.get(key))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
    };
    Ok(OpenAIChatRequest {
        model: String::new(), // 由 server 用路径中的模型名回填
        messages: serde_json::from_value(Value::Array(messages))
            .map_err(|e| format!("Invalid contents: {e}"))?,
        temperature: num("temperature"),
        max_tokens: num("maxOutputTokens").map(|v| v as u32),
        stream: None, // 流式与否由路径（:streamGenerateContent）决定
        stop: cfg
            .and_then(|c| c.get("stopSequences"))
            .and_then(|s| s.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }),
        top_p: num("topP"),
        frequency_penalty: None,
        presence_penalty: None,
    })
}

/// 内部 OpenAIChat 响应 → Gemini generateContent 响应（客户端方向）
pub fn openai_to_gemini_response(resp: &Value) -> Value {
    let content = resp
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let finish = match resp
        .pointer("/choices/0/finish_reason")
        .and_then(|f| f.as_str())
    {
        Some("length") => "MAX_TOKENS",
        Some("content_filter") => "SAFETY",
        _ => "STOP",
    };
    json!({
        "candidates": [{
            "content": { "role": "model", "parts": [{ "text": content }] },
            "finishReason": finish,
            "index": 0,
        }],
        "usageMetadata": {
            "promptTokenCount": resp.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            "candidatesTokenCount": resp.pointer("/usage/completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            "totalTokenCount": resp.pointer("/usage/total_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        },
        "modelVersion": resp.get("model").and_then(|m| m.as_str()).unwrap_or(""),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_hub::types::ChatMessage;

    fn req() -> OpenAIChatRequest {
        OpenAIChatRequest {
            model: "gemini-2.5-flash".into(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: "be brief".into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: "hello".into(),
                },
            ],
            temperature: Some(0.5),
            max_tokens: Some(128),
            stream: None,
            stop: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
        }
    }

    #[test]
    fn test_openai_to_gemini() {
        let g = openai_to_gemini(&req());
        assert_eq!(
            g.pointer("/systemInstruction/parts/0/text")
                .and_then(|v| v.as_str()),
            Some("be brief")
        );
        assert_eq!(
            g.pointer("/contents/0/role").and_then(|v| v.as_str()),
            Some("user")
        );
        assert_eq!(
            g.pointer("/contents/0/parts/0/text")
                .and_then(|v| v.as_str()),
            Some("hello")
        );
        assert_eq!(
            g.pointer("/generationConfig/temperature")
                .and_then(|v| v.as_f64()),
            Some(0.5)
        );
        assert_eq!(
            g.pointer("/generationConfig/maxOutputTokens")
                .and_then(|v| v.as_u64()),
            Some(128)
        );
    }

    #[test]
    fn test_gemini_to_openai() {
        let resp = json!({
            "candidates": [{
                "content": { "role": "model", "parts": [{ "text": "hi there" }] },
                "finishReason": "STOP"
            }],
            "usageMetadata": { "promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15 }
        });
        let oai = gemini_to_openai("id-1", "gemini-2.5-flash", &resp);
        assert_eq!(
            oai.pointer("/choices/0/message/content")
                .and_then(|v| v.as_str()),
            Some("hi there")
        );
        assert_eq!(
            oai.pointer("/choices/0/finish_reason")
                .and_then(|v| v.as_str()),
            Some("stop")
        );
        assert_eq!(
            oai.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()),
            Some(10)
        );
        assert_eq!(
            oai.pointer("/usage/completion_tokens")
                .and_then(|v| v.as_u64()),
            Some(5)
        );
    }

    #[test]
    fn test_gemini_stream_chunks() {
        let mut started = false;
        let c1 = json!({
            "candidates": [{ "content": { "parts": [{ "text": "Hel" }] } }]
        });
        let lines = gemini_chunk_to_openai_lines(&c1, "gemini-2.5-flash", "id-1", &mut started);
        assert!(started);
        assert!(lines[0].contains("\"role\":\"assistant\""));
        assert!(lines[0].contains("\"content\":\"Hel\""));

        let c2 = json!({
            "candidates": [{ "content": { "parts": [{ "text": "lo" }] }, "finishReason": "STOP" }],
            "usageMetadata": { "promptTokenCount": 3, "candidatesTokenCount": 2, "totalTokenCount": 5 }
        });
        let lines = gemini_chunk_to_openai_lines(&c2, "gemini-2.5-flash", "id-1", &mut started);
        let joined = lines.join("\n");
        assert!(joined.contains("\"content\":\"lo\""));
        assert!(joined.contains("\"finish_reason\":\"stop\""));
        assert!(joined.contains("\"prompt_tokens\":3"));
        assert!(joined.contains("data: [DONE]"));
    }
}
