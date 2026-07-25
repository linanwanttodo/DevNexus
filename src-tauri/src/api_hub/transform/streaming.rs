//! Chunk-level SSE format conversion between protocols.
//!
//! Converts individual SSE `data:` lines between OpenAI Chat, OpenAI Responses,
//! and Anthropic streaming formats.

use serde_json::Value;

/// Direction of streaming format conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDirection {
    /// OpenAI Chat chunks → Anthropic SSE events
    OpenAIChatToAnthropic,
    /// Anthropic SSE events → OpenAI Chat chunks
    AnthropicToOpenAIChat,
    /// OpenAI Chat chunks → OpenAI Responses events
    OpenAIChatToResponses,
    /// OpenAI Responses events → OpenAI Chat chunks
    ResponsesToOpenAIChat,
}

/// State tracker for multi-event protocol conversions.
#[derive(Debug, Default)]
pub struct StreamState {
    /// Whether we've emitted the initial message_start (Anthropic) or response.created (Responses)
    pub started: bool,
    /// Whether content block has been opened (Anthropic)
    pub content_block_opened: bool,
    /// Accumulated model name from first chunk
    pub model: String,
    /// Accumulated response id
    pub id: String,
}

/// Transform a single raw SSE line (the full line including `data: ` prefix or `event: ` prefix).
/// Returns zero or more output lines to emit to the client.
///
/// For Anthropic output, lines may include `event:` + `data:` pairs.
/// Returns `None` for lines that should be skipped (comments, empty keepalive).
pub fn transform_sse_line(
    direction: StreamDirection,
    line: &str,
    state: &mut StreamState,
) -> Vec<String> {
    match direction {
        StreamDirection::OpenAIChatToAnthropic => openai_chat_to_anthropic(line, state),
        StreamDirection::AnthropicToOpenAIChat => anthropic_to_openai_chat(line, state),
        StreamDirection::OpenAIChatToResponses => openai_chat_to_responses(line, state),
        StreamDirection::ResponsesToOpenAIChat => responses_to_openai_chat(line, state),
    }
}

// ── OpenAI Chat → Anthropic ──────────────────────────────────

fn openai_chat_to_anthropic(line: &str, state: &mut StreamState) -> Vec<String> {
    let data = match extract_data_field(line) {
        Some(d) => d,
        None => return vec![],
    };

    if data == "[DONE]" {
        // Emit content_block_stop + message_delta + message_stop
        let mut out = vec![];
        if state.content_block_opened {
            out.push("event: content_block_stop".to_string());
            out.push(format!("data: {}", serde_json::json!({"type": "content_block_stop", "index": 0})));
            out.push(String::new());
        }
        out.push("event: message_delta".to_string());
        out.push(format!("data: {}", serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"output_tokens": 0}
        })));
        out.push(String::new());
        out.push("event: message_stop".to_string());
        out.push(format!("data: {}", serde_json::json!({"type": "message_stop"})));
        out.push(String::new());
        return out;
    }

    let chunk: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut out = vec![];

    // Emit message_start on first chunk
    if !state.started {
        state.started = true;
        state.id = chunk.get("id").and_then(|v| v.as_str()).unwrap_or("msg_stream").to_string();
        state.model = chunk.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string();

        out.push("event: message_start".to_string());
        out.push(format!("data: {}", serde_json::json!({
            "type": "message_start",
            "message": {
                "id": state.id,
                "type": "message",
                "role": "assistant",
                "model": state.model,
                "content": [],
                "stop_reason": null,
                "usage": {"input_tokens": 0, "output_tokens": 0}
            }
        })));
        out.push(String::new());
    }

    // Extract delta content
    let delta = chunk.pointer("/choices/0/delta");
    let finish_reason = chunk.pointer("/choices/0/finish_reason");

    if let Some(delta_obj) = delta {
        // If there's content text, emit content_block_start (once) + content_block_delta
        if let Some(text) = delta_obj.get("content").and_then(|c| c.as_str()) {
            if !state.content_block_opened {
                state.content_block_opened = true;
                out.push("event: content_block_start".to_string());
                out.push(format!("data: {}", serde_json::json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": {"type": "text", "text": ""}
                })));
                out.push(String::new());
            }

            if !text.is_empty() {
                out.push("event: content_block_delta".to_string());
                out.push(format!("data: {}", serde_json::json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {"type": "text_delta", "text": text}
                })));
                out.push(String::new());
            }
        }
    }

    // Handle finish_reason in the same chunk (some providers send it with content)
    if let Some(reason) = finish_reason.and_then(|r| r.as_str()) {
        let stop_reason = match reason {
            "length" => "max_tokens",
            "tool_calls" => "tool_use",
            _ => "end_turn",
        };
        if state.content_block_opened {
            out.push("event: content_block_stop".to_string());
            out.push(format!("data: {}", serde_json::json!({"type": "content_block_stop", "index": 0})));
            out.push(String::new());
            state.content_block_opened = false;
        }
        out.push("event: message_delta".to_string());
        out.push(format!("data: {}", serde_json::json!({
            "type": "message_delta",
            "delta": {"stop_reason": stop_reason},
            "usage": {"output_tokens": 0}
        })));
        out.push(String::new());
        out.push("event: message_stop".to_string());
        out.push(format!("data: {}", serde_json::json!({"type": "message_stop"})));
        out.push(String::new());
    }

    out
}

// ── Anthropic → OpenAI Chat ──────────────────────────────────

fn anthropic_to_openai_chat(line: &str, state: &mut StreamState) -> Vec<String> {
    // Anthropic uses `event: xxx\ndata: {...}` format.
    // We only process `data:` lines; `event:` lines are informational.
    let data = match extract_data_field(line) {
        Some(d) => d,
        None => return vec![],
    };

    let event: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match event_type {
        "message_start" => {
            // Extract model and id, emit first chunk with role
            let msg = event.get("message").unwrap_or(&Value::Null);
            state.id = msg.get("id").and_then(|v| v.as_str()).unwrap_or("chatcmpl-stream").to_string();
            state.model = msg.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string();
            state.started = true;

            let chunk = serde_json::json!({
                "id": state.id,
                "object": "chat.completion.chunk",
                "created": chrono::Utc::now().timestamp(),
                "model": state.model,
                "choices": [{"index": 0, "delta": {"role": "assistant", "content": ""}, "finish_reason": null}]
            });
            vec![format!("data: {}", chunk), String::new()]
        }
        "content_block_delta" => {
            let text = event.pointer("/delta/text").and_then(|t| t.as_str()).unwrap_or("");
            if text.is_empty() {
                return vec![];
            }
            let chunk = serde_json::json!({
                "id": state.id,
                "object": "chat.completion.chunk",
                "created": chrono::Utc::now().timestamp(),
                "model": state.model,
                "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}]
            });
            vec![format!("data: {}", chunk), String::new()]
        }
        "message_delta" => {
            let stop_reason = event.pointer("/delta/stop_reason").and_then(|r| r.as_str());
            let finish_reason = match stop_reason {
                Some("max_tokens") => "length",
                Some("tool_use") => "tool_calls",
                _ => "stop",
            };
            let chunk = serde_json::json!({
                "id": state.id,
                "object": "chat.completion.chunk",
                "created": chrono::Utc::now().timestamp(),
                "model": state.model,
                "choices": [{"index": 0, "delta": {}, "finish_reason": finish_reason}]
            });
            // Also check for usage in message_delta
            let mut out = vec![format!("data: {}", chunk), String::new()];
            // Append [DONE] after finish
            out.push("data: [DONE]".to_string());
            out.push(String::new());
            out
        }
        "message_stop" => {
            // Already handled via message_delta, skip
            vec![]
        }
        _ => vec![],
    }
}

// ── OpenAI Chat → Responses ──────────────────────────────────

fn openai_chat_to_responses(line: &str, state: &mut StreamState) -> Vec<String> {
    let data = match extract_data_field(line) {
        Some(d) => d,
        None => return vec![],
    };

    if data == "[DONE]" {
        let mut out = vec![];
        out.push(format!("data: {}", serde_json::json!({
            "type": "response.output_item.done",
            "output_index": 0,
            "item": {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": ""}]}
        })));
        out.push(String::new());
        out.push(format!("data: {}", serde_json::json!({
            "type": "response.completed",
            "response": {"id": state.id, "status": "completed"}
        })));
        out.push(String::new());
        return out;
    }

    let chunk: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut out = vec![];

    if !state.started {
        state.started = true;
        state.id = chunk.get("id").and_then(|v| v.as_str()).unwrap_or("resp_stream").to_string();
        state.model = chunk.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string();

        // Emit response.created + response.in_progress + output_item.added + content_part.added
        out.push(format!("data: {}", serde_json::json!({
            "type": "response.created",
            "response": {"id": state.id, "object": "response", "model": state.model, "status": "in_progress"}
        })));
        out.push(String::new());
        out.push(format!("data: {}", serde_json::json!({
            "type": "response.in_progress",
            "response": {"id": state.id, "status": "in_progress"}
        })));
        out.push(String::new());
        out.push(format!("data: {}", serde_json::json!({
            "type": "response.output_item.added",
            "output_index": 0,
            "item": {"type": "message", "role": "assistant", "content": []}
        })));
        out.push(String::new());
        out.push(format!("data: {}", serde_json::json!({
            "type": "response.content_part.added",
            "output_index": 0,
            "content_index": 0,
            "part": {"type": "output_text", "text": ""}
        })));
        out.push(String::new());
    }

    // Extract delta content
    let delta = chunk.pointer("/choices/0/delta");
    let finish_reason = chunk.pointer("/choices/0/finish_reason").and_then(|r| r.as_str());

    if let Some(delta_obj) = delta {
        if let Some(text) = delta_obj.get("content").and_then(|c| c.as_str()) {
            if !text.is_empty() {
                out.push(format!("data: {}", serde_json::json!({
                    "type": "response.output_text.delta",
                    "output_index": 0,
                    "content_index": 0,
                    "delta": text
                })));
                out.push(String::new());
            }
        }
    }

    if let Some(_reason) = finish_reason {
        out.push(format!("data: {}", serde_json::json!({
            "type": "response.output_text.done",
            "output_index": 0,
            "content_index": 0,
            "text": ""
        })));
        out.push(String::new());
        out.push(format!("data: {}", serde_json::json!({
            "type": "response.output_item.done",
            "output_index": 0,
            "item": {"type": "message", "role": "assistant"}
        })));
        out.push(String::new());
        out.push(format!("data: {}", serde_json::json!({
            "type": "response.completed",
            "response": {"id": state.id, "model": state.model, "status": "completed"}
        })));
        out.push(String::new());
    }

    out
}

// ── Responses → OpenAI Chat ──────────────────────────────────

fn responses_to_openai_chat(line: &str, state: &mut StreamState) -> Vec<String> {
    let data = match extract_data_field(line) {
        Some(d) => d,
        None => return vec![],
    };

    let event: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match event_type {
        "response.created" | "response.in_progress" => {
            if !state.started {
                state.started = true;
                let resp = event.get("response").unwrap_or(&Value::Null);
                state.id = resp.get("id").and_then(|v| v.as_str()).unwrap_or("chatcmpl-stream").to_string();
                state.model = resp.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string();

                let chunk = serde_json::json!({
                    "id": state.id,
                    "object": "chat.completion.chunk",
                    "created": chrono::Utc::now().timestamp(),
                    "model": state.model,
                    "choices": [{"index": 0, "delta": {"role": "assistant", "content": ""}, "finish_reason": null}]
                });
                return vec![format!("data: {}", chunk), String::new()];
            }
            vec![]
        }
        "response.output_text.delta" => {
            let text = event.get("delta").and_then(|d| d.as_str()).unwrap_or("");
            if text.is_empty() {
                return vec![];
            }
            let chunk = serde_json::json!({
                "id": state.id,
                "object": "chat.completion.chunk",
                "created": chrono::Utc::now().timestamp(),
                "model": state.model,
                "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}]
            });
            vec![format!("data: {}", chunk), String::new()]
        }
        "response.completed" => {
            let chunk = serde_json::json!({
                "id": state.id,
                "object": "chat.completion.chunk",
                "created": chrono::Utc::now().timestamp(),
                "model": state.model,
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
            });
            let mut out = vec![format!("data: {}", chunk), String::new()];
            out.push("data: [DONE]".to_string());
            out.push(String::new());
            out
        }
        _ => vec![],
    }
}

// ── Helpers ──────────────────────────────────────────────────

/// Extract the payload after `data: ` prefix. Returns None for non-data lines.
fn extract_data_field(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with(':') {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("data: ") {
        Some(rest)
    } else if let Some(rest) = trimmed.strip_prefix("data:") {
        Some(rest.trim_start())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_to_anthropic_content_delta() {
        let mut state = StreamState::default();
        // First chunk with role
        let line1 = r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}"#;
        let out1 = transform_sse_line(StreamDirection::OpenAIChatToAnthropic, line1, &mut state);
        assert!(state.started);
        // Should contain message_start
        assert!(out1.iter().any(|l| l.contains("message_start")));

        // Content chunk
        let line2 = r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let out2 = transform_sse_line(StreamDirection::OpenAIChatToAnthropic, line2, &mut state);
        assert!(out2.iter().any(|l| l.contains("content_block_delta")));
        assert!(out2.iter().any(|l| l.contains("Hello")));
    }

    #[test]
    fn test_anthropic_to_openai_content_delta() {
        let mut state = StreamState::default();
        // message_start
        let line1 = r#"data: {"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","model":"claude-3","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0}}}"#;
        let out1 = transform_sse_line(StreamDirection::AnthropicToOpenAIChat, line1, &mut state);
        assert!(state.started);
        assert!(out1.iter().any(|l| l.contains("chat.completion.chunk")));

        // content_block_delta
        let line2 = r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#;
        let out2 = transform_sse_line(StreamDirection::AnthropicToOpenAIChat, line2, &mut state);
        assert!(out2.iter().any(|l| l.contains("\"content\":\"Hi\"")));
    }

    #[test]
    fn test_openai_to_responses_delta() {
        let mut state = StreamState::default();
        let line1 = r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}"#;
        let out1 = transform_sse_line(StreamDirection::OpenAIChatToResponses, line1, &mut state);
        assert!(state.started);
        assert!(out1.iter().any(|l| l.contains("response.created")));

        let line2 = r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"World"},"finish_reason":null}]}"#;
        let out2 = transform_sse_line(StreamDirection::OpenAIChatToResponses, line2, &mut state);
        assert!(out2.iter().any(|l| l.contains("response.output_text.delta")));
        assert!(out2.iter().any(|l| l.contains("World")));
    }

    #[test]
    fn test_done_signal() {
        let mut state = StreamState { started: true, content_block_opened: true, ..Default::default() };
        let out = transform_sse_line(StreamDirection::OpenAIChatToAnthropic, "data: [DONE]", &mut state);
        assert!(out.iter().any(|l| l.contains("message_stop")));
    }
}
