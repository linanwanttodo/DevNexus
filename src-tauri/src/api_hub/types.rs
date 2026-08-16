use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::crypto::ApiKeyCipher;

// ── Provider ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    /// 单一协议字段：同时决定品牌预设、线协议、认证方式、上游端点与 token 字段
    pub protocol: ApiProtocol,
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
    pub model_aliases: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub model_context_lengths: std::collections::HashMap<String, u64>,
    pub enabled: bool,
    pub created_at: i64,
}

/// 从 Provider API 获取的模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedModel {
    pub id: String,
    pub name: String,
    pub owned_by: Option<String>,
    pub enabled: bool,
}

/// API 协议：一个选项同时锁定品牌预设、线协议、认证方式、上游端点与 token 字段
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApiProtocol {
    /// OpenAI Chat Completions（/v1/chat/completions）
    #[serde(rename = "openai_chat")]
    OpenAIChat,
    /// OpenAI Responses（/v1/responses）
    #[serde(rename = "openai_responses")]
    OpenAIResponses,
    /// Anthropic Messages（/v1/messages）
    #[serde(rename = "anthropic")]
    Anthropic,
    /// Google Gemini（路径含模型名 /v1beta/models/{model}:generateContent，
    /// 实际上游端点由 server 侧按模型生成）
    #[serde(rename = "gemini")]
    Gemini,
}

/// token 用量字段风格
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenScheme {
    /// prompt_tokens + completion_tokens (OpenAI)
    PromptCompletion,
    /// input_tokens + output_tokens (Anthropic / OpenAI Responses)
    InputOutput,
    /// usageMetadata.promptTokenCount + candidatesTokenCount (Gemini)
    GeminiMetadata,
}

impl ApiProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiProtocol::OpenAIChat => "openai_chat",
            ApiProtocol::OpenAIResponses => "openai_responses",
            ApiProtocol::Anthropic => "anthropic",
            ApiProtocol::Gemini => "gemini",
        }
    }

    pub fn from_protocol_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai_chat" => Some(ApiProtocol::OpenAIChat),
            "openai_responses" | "responses" => Some(ApiProtocol::OpenAIResponses),
            "anthropic" => Some(ApiProtocol::Anthropic),
            "gemini" | "google_gemini" => Some(ApiProtocol::Gemini),
            _ => None,
        }
    }

    /// 上游请求端点（路径部分）。
    /// Gemini 的真实路径含模型名，此处为占位串，实际由
    /// server::upstream_endpoint 按模型生成。
    pub fn endpoint(&self) -> &'static str {
        match self {
            ApiProtocol::OpenAIChat => "/v1/chat/completions",
            ApiProtocol::OpenAIResponses => "/v1/responses",
            ApiProtocol::Anthropic => "/v1/messages",
            ApiProtocol::Gemini => "/v1beta/models/{model}:generateContent",
        }
    }

    /// token 用量提取风格
    pub fn token_scheme(&self) -> TokenScheme {
        match self {
            ApiProtocol::OpenAIChat => TokenScheme::PromptCompletion,
            ApiProtocol::OpenAIResponses | ApiProtocol::Anthropic => TokenScheme::InputOutput,
            ApiProtocol::Gemini => TokenScheme::GeminiMetadata,
        }
    }
}

// ── API Request/Response formats ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: serde_json::Value, // String or Vec<ContentBlock>
}

/// 多模态 content block（预留给 image/tool 扩展）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<String>,
    pub image_url: Option<ImageUrl>,
    pub source: Option<ImageSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ImageUrl {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

/// OpenAI /v1/chat/completions 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
    pub stop: Option<Vec<String>>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
}

/// OpenAI /v1/chat/completions 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Anthropic /v1/messages 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub system: Option<String>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub stream: Option<bool>,
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: serde_json::Value, // String or Vec<ContentBlock>
}

/// Anthropic /v1/messages 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicResponse {
    pub id: String,
    pub model: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub content: Vec<AnthropicContent>,
    pub usage: Option<AnthropicUsage>,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// ── Request Log ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLog {
    pub id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub model: String,
    pub request_model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub latency_ms: u64,
    pub status_code: u16,
    pub error_message: Option<String>,
    pub timestamp: i64,
    pub is_streaming: bool,
}

// ── Shared State ──────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub providers: Arc<tokio::sync::RwLock<Vec<Provider>>>,
    pub request_logs: Arc<tokio::sync::RwLock<VecDeque<RequestLog>>>,
    pub db: Arc<tokio::sync::Mutex<Option<rusqlite::Connection>>>,
    pub http_client: reqwest::Client,
    pub running: Arc<AtomicBool>,
    pub api_key_cipher: Arc<ApiKeyCipher>,
    /// 本地 HTTP 服务访问令牌（H1 安全修复）：外部客户端必须携带
    /// `X-DevNexus-Token` 或 `Authorization: Bearer <token>` 才能访问代理端点。
    /// 防止本机任意进程 / 恶意网页（DNS rebinding、CSRF 式请求）
    /// 盗用已配置 Provider 的 API Key。
    pub auth_token: String,
}
