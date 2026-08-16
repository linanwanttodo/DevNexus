//! Ollama 原生协议转换（/api/chat，NDJSON 流式）：
//! 内部 OpenAIChat 格式 ↔ Ollama 格式。
//!
//! Ollama 要点：
//! - 请求：{model, messages[{role, content}], stream, options{temperature, top_p, num_predict}}
//! - 响应：{model, created_at, message{role, content}, done, done_reason?,
//!   prompt_eval_count?, eval_count?}
//! - 流式：NDJSON（每行一个 JSON，无 data: 前缀），最后一行 done:true 带用量

use serde_json::{json, Value};

use super::super::types::OpenAIChatRequest;

/// Ollama /api/chat 请求 → 内部 OpenAIChat 请求（客户端方向）
pub fn ollama_to_openai_req(body: &Value) -> Result<OpenAIChatRequest, String> {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .ok_or("Ollama request missing 'model' field")?
        .to_string();
    let messages = body
        .get("messages")
        .and_then(|m| m.as_array())
        .ok_or("Ollama request missing 'messages' array")?
        .iter()
        .map(|m| {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            // Ollama 支持 system 角色，直接透传
            json!({ "role": role, "content": m.get("content").and_then(|c| c.as_str()).unwrap_or("") })
        })
        .collect::<Vec<_>>();
    if messages.is_empty() {
        return Err("Ollama request has empty 'messages'".to_string());
    }
    let options = body.get("options");
    let num_to_f32 = |key: &str| -> Option<f32> {
        options
            .and_then(|o| o.get(key))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
    };
    Ok(OpenAIChatRequest {
        model,
        // 反序列化为 OpenAIChatRequest 需要 String content
        messages: serde_json::from_value(Value::Array(messages))
            .map_err(|e| format!("Invalid messages: {e}"))?,
        temperature: num_to_f32("temperature"),
        max_tokens: num_to_f32("num_predict").map(|v| v as u32),
        stream: body.get("stream").and_then(|s| s.as_bool()),
        stop: body.get("stop").and_then(|s| s.as_array()).map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        }),
        top_p: num_to_f32("top_p"),
        frequency_penalty: None,
        presence_penalty: None,
    })
}

/// 内部 OpenAIChat 请求 → Ollama /api/chat 请求（供应商方向）
pub fn openai_to_ollama_req(req: &OpenAIChatRequest) -> Value {
    let mut options = serde_json::Map::new();
    if let Some(t) = req.temperature {
        options.insert("temperature".into(), json!(t));
    }
    if let Some(t) = req.top_p {
        options.insert("top_p".into(), json!(t));
    }
    if let Some(m) = req.max_tokens {
        options.insert("num_predict".into(), json!(m));
    }
    if let Some(s) = &req.stop {
        options.insert("stop".into(), json!(s));
    }

    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            json!({
                "role": m.role,
                "content": match &m.content {
                    Value::String(s) => s.clone(),
                    Value::Array(parts) => parts
                        .iter()
                        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join(""),
                    _ => String::new(),
                },
            })
        })
        .collect();

    let mut out = json!({
        "model": req.model,
        "messages": messages,
        "stream": req.stream.unwrap_or(false),
    });
    if !options.is_empty() {
        out["options"] = Value::Object(options);
    }
    out
}

fn finish_of(done_reason: Option<&str>) -> &'static str {
    match done_reason {
        Some("length") => "length",
        _ => "stop",
    }
}

/// Ollama /api/chat 响应 → 内部 OpenAIChat 响应
pub fn ollama_to_openai(id: &str, resp: &Value) -> Value {
    let content = resp
        .pointer("/message/content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let finish = finish_of(resp.get("done_reason").and_then(|d| d.as_str()));
    json!({
        "id": id,
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": resp.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": finish,
        }],
        "usage": {
            "prompt_tokens": resp.get("prompt_eval_count").and_then(|v| v.as_u64()).unwrap_or(0),
            "completion_tokens": resp.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0),
            "total_tokens":
                resp.get("prompt_eval_count").and_then(|v| v.as_u64()).unwrap_or(0)
                + resp.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0),
        },
    })
}

/// 内部 OpenAIChat 响应 → Ollama /api/chat 响应（客户端方向）
pub fn openai_to_ollama_response(resp: &Value) -> Value {
    let content = resp
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let finish = resp
        .pointer("/choices/0/finish_reason")
        .and_then(|f| f.as_str())
        .unwrap_or("stop");
    json!({
        "model": resp.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "created_at": chrono::Utc::now().to_rfc3339(),
        "message": { "role": "assistant", "content": content },
        "done": true,
        "done_reason": if finish == "length" { json!("length") } else { Value::Null },
        "prompt_eval_count": resp.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        "eval_count": resp.pointer("/usage/completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
    })
}

/// Ollama NDJSON 流行 → OpenAI chunk 行（供应商方向）。
/// `started` 由调用方维护（首个内容块补 role）。
pub fn ollama_chunk_to_openai_lines(line: &str, model: &str, started: &mut bool) -> Vec<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return vec![];
    }
    let chunk: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let id = "chatcmpl-ollama-stream";
    let done = chunk.get("done").and_then(|d| d.as_bool()).unwrap_or(false);
    let text = chunk
        .pointer("/message/content")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    let mut out = Vec::new();
    if !text.is_empty() {
        let mut delta = serde_json::Map::new();
        if !*started {
            delta.insert("role".into(), json!("assistant"));
            *started = true;
        }
        delta.insert("content".into(), json!(text));
        let c = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": chrono::Utc::now().timestamp(),
            "model": chunk.get("model").and_then(|m| m.as_str()).unwrap_or(model),
            "choices": [{ "index": 0, "delta": Value::Object(delta), "finish_reason": null }],
        });
        out.push(format!("data: {c}"));
        out.push(String::new());
    }
    if done {
        let finish = finish_of(chunk.get("done_reason").and_then(|d| d.as_str()));
        let c = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": chrono::Utc::now().timestamp(),
            "model": chunk.get("model").and_then(|m| m.as_str()).unwrap_or(model),
            "choices": [{ "index": 0, "delta": {}, "finish_reason": finish }],
        });
        out.push(format!("data: {c}"));
        out.push(String::new());
        // done 行携带用量时附 usage chunk
        if chunk.get("prompt_eval_count").is_some() || chunk.get("eval_count").is_some() {
            let u = json!({
                "id": id,
                "object": "chat.completion.chunk",
                "created": chrono::Utc::now().timestamp(),
                "model": chunk.get("model").and_then(|m| m.as_str()).unwrap_or(model),
                "choices": [],
                "usage": {
                    "prompt_tokens": chunk.get("prompt_eval_count").and_then(|v| v.as_u64()).unwrap_or(0),
                    "completion_tokens": chunk.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0),
                },
            });
            out.push(format!("data: {u}"));
            out.push(String::new());
        }
        out.push("data: [DONE]".to_string());
        out.push(String::new());
    }
    out
}

/// OpenAI chunk SSE 行 → Ollama NDJSON 行（客户端方向）。
/// `started` 由调用方维护（首个 delta 补 role）。
pub fn openai_chunk_to_ollama_lines(line: &str, model: &str, started: &mut bool) -> Vec<String> {
    let data = match line.strip_prefix("data:") {
        Some(d) => d.trim(),
        None => return vec![], // 非数据行（注释/event 行）跳过
    };
    if data.is_empty() || data == "[DONE]" {
        return vec![];
    }
    let chunk: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    // usage 独立 chunk（choices 为空）转换为带用量的 done 行
    if chunk.get("usage").is_some()
        && chunk
            .get("choices")
            .and_then(|c| c.as_array())
            .map(|a| a.is_empty())
            .unwrap_or(true)
    {
        return vec![serde_json::to_string(&json!({
            "model": model,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "message": { "role": "assistant", "content": "" },
            "done": true,
            "prompt_eval_count": chunk.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            "eval_count": chunk.pointer("/usage/completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        }))
        .unwrap_or_default()];
    }

    let delta = chunk
        .pointer("/choices/0/delta")
        .cloned()
        .unwrap_or(Value::Null);
    let finish = chunk
        .pointer("/choices/0/finish_reason")
        .and_then(|f| f.as_str())
        .map(String::from);

    let mut out = Vec::new();
    let text = delta.get("content").and_then(|c| c.as_str()).unwrap_or("");
    if !text.is_empty() {
        let mut msg = serde_json::Map::new();
        msg.insert("role".into(), json!("assistant"));
        msg.insert("content".into(), json!(text));
        let _ = *started; // role 始终带在 message 里，Ollama 客户端兼容
        out.push(
            serde_json::to_string(&json!({
                "model": model,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "message": Value::Object(msg),
                "done": false,
            }))
            .unwrap_or_default(),
        );
    }
    if let Some(f) = finish {
        out.push(
            serde_json::to_string(&json!({
                "model": model,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "message": { "role": "assistant", "content": "" },
                "done": true,
                "done_reason": if f == "length" { json!("length") } else { Value::Null },
            }))
            .unwrap_or_default(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_req_roundtrip() {
        let body = json!({
            "model": "llama3",
            "stream": false,
            "messages": [
                {"role": "system", "content": "be nice"},
                {"role": "user", "content": "hi"}
            ],
            "options": { "temperature": 0.7, "num_predict": 64 }
        });
        let req = ollama_to_openai_req(&body).unwrap();
        assert_eq!(req.model, "llama3");
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, "system");
        assert_eq!(req.temperature, Some(0.7));
        assert_eq!(req.max_tokens, Some(64));

        let back = openai_to_ollama_req(&req);
        assert_eq!(
            back.pointer("/model").and_then(|v| v.as_str()),
            Some("llama3")
        );
        assert_eq!(
            back.pointer("/messages/1/content").and_then(|v| v.as_str()),
            Some("hi")
        );
        assert_eq!(
            back.pointer("/options/num_predict")
                .and_then(|v| v.as_u64()),
            Some(64)
        );
    }

    #[test]
    fn test_ollama_response_conversion() {
        let resp = json!({
            "model": "llama3",
            "message": { "role": "assistant", "content": "hey" },
            "done": true,
            "prompt_eval_count": 9,
            "eval_count": 4
        });
        let oai = ollama_to_openai("id-x", &resp);
        assert_eq!(
            oai.pointer("/choices/0/message/content")
                .and_then(|v| v.as_str()),
            Some("hey")
        );
        assert_eq!(
            oai.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()),
            Some(9)
        );
        let back = openai_to_ollama_response(&oai);
        assert_eq!(
            back.pointer("/message/content").and_then(|v| v.as_str()),
            Some("hey")
        );
        assert_eq!(
            back.pointer("/eval_count").and_then(|v| v.as_u64()),
            Some(4)
        );
        assert_eq!(back.pointer("/done").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn test_ollama_stream_lines() {
        let mut started = false;
        let out = ollama_chunk_to_openai_lines(
            r#"{"model":"llama3","message":{"role":"assistant","content":"He"},"done":false}"#,
            "llama3",
            &mut started,
        );
        assert!(out[0].contains("\"role\":\"assistant\""));
        assert!(out[0].contains("\"content\":\"He\""));

        let out = ollama_chunk_to_openai_lines(
            r#"{"model":"llama3","message":{"role":"assistant","content":"y"},"done":true,"done_reason":"stop","prompt_eval_count":5,"eval_count":2}"#,
            "llama3",
            &mut started,
        );
        let joined = out.join("\n");
        assert!(joined.contains("\"finish_reason\":\"stop\""));
        assert!(joined.contains("\"prompt_tokens\":5"));
        assert!(joined.contains("data: [DONE]"));
    }

    #[test]
    fn test_openai_stream_to_ollama() {
        let mut started = false;
        let out = openai_chunk_to_ollama_lines(
            r#"data: {"id":"c1","object":"chat.completion.chunk","model":"m","choices":[{"index":0,"delta":{"role":"assistant","content":"Ho"},"finish_reason":null}]}"#,
            "m",
            &mut started,
        );
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("\"content\":\"Ho\""));
        assert!(out[0].contains("\"done\":false"));

        let out = openai_chunk_to_ollama_lines(
            r#"data: {"id":"c1","object":"chat.completion.chunk","model":"m","choices":[{"index":0,"delta":{},"finish_reason":"length"}]}"#,
            "m",
            &mut started,
        );
        assert_eq!(out.len(), 1);
        assert!(out[0].contains("\"done\":true"));
        assert!(out[0].contains("\"done_reason\":\"length\""));

        // [DONE] 与 usage chunk
        assert!(openai_chunk_to_ollama_lines("data: [DONE]", "m", &mut started).is_empty());
        let usage_line = r#"data: {"id":"c1","object":"chat.completion.chunk","model":"m","choices":[],"usage":{"prompt_tokens":8,"completion_tokens":3}}"#;
        let out = openai_chunk_to_ollama_lines(usage_line, "m", &mut started);
        assert!(out[0].contains("\"prompt_eval_count\":8"));
    }
}
