# API Hub — 模块设计文档

## 1. 功能概述

API Hub 是 DevNexus 内部的本地 AI 统一网关，将多个 AI 模型服务的 API 聚合为一个本地 HTTP 端口，并实现不同协议之间的双向格式转换。第三方应用（VS Code、Cursor、CLI 工具等）只需配置一个地址即可访问所有已配置的 Provider 的模型。

**设计目标**: 让用户无需在多个 API Key 和不同 API 格式之间反复切换，所有模型请求统一走 `localhost:3456/v1`。

**通信链路**:
```
第三方应用 (curl/Cursor/VS Code) ──→ localhost:3456/v1/chat/completions
                              ──→ localhost:3456/v1/responses
                              ──→ localhost:3456/v1/messages

DevNexus 前端页面 ──→ Tauri Command ──→ api_hub/commands.rs
```

---

## 2. 数据结构

### 2.1 Provider

```rust
/// API Provider 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,                                          // UUID v4
    pub name: String,                                        // 显示名称
    pub protocol: ApiProtocol,                               // 协议类型
    pub base_url: String,                                    // 上游 base URL
    pub api_key: String,                                     // API Key
    pub models: Vec<String>,                                 // 可用模型列表
    pub model_aliases: HashMap<String, String>,               // 模型别名映射
    pub model_context_lengths: HashMap<String, u64>,           // 各模型上下文长度
    pub enabled: bool,                                       // 是否启用
    pub created_at: i64,                                     // 创建时间戳
}
```

### 2.2 ApiProtocol 枚举

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiProtocol {
    OpenAIChat,       // OpenAI Chat Completions (/v1/chat/completions)
    OpenAIResponses,  // OpenAI Responses (/v1/responses)
    Anthropic,        // Anthropic Messages (/v1/messages)
}
```

单一协议字段同时决定以下维度:

| 维度 | 说明 |
|------|------|
| 上游端点 (endpoint) | 如 OpenAIChat → `/v1/chat/completions` |
| 认证方式 | OpenAI: `Authorization: Bearer`; Anthropic: `x-api-key` + `anthropic-version` |
| Token 提取方案 | `PromptCompletion` (OpenAI) / `InputOutput` (Anthropic / Responses) |
| 格式转换策略 | 确定请求/响应转换的目标协议 |

### 2.3 请求格式

```rust
/// OpenAI ChatCompletion 请求
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

/// Chat 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,              // "system" / "user" / "assistant"
    pub content: serde_json::Value, // String 或 Vec<ContentBlock>
}

/// Anthropic Messages 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub system: Option<String>,       // Anthropic 独立 system 参数
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub stream: Option<bool>,
    pub stop_sequences: Option<Vec<String>>,
}
```

### 2.4 响应格式

```rust
/// OpenAI ChatCompletion 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    pub usage: Option<OpenAIUsage>,
}

/// Anthropic Messages 响应
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
```

### 2.5 请求日志

```rust
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
```

### 2.6 共享状态

```rust
#[derive(Clone)]
pub struct AppState {
    pub providers: Arc<tokio::sync::RwLock<Vec<Provider>>>,
    pub request_logs: Arc<tokio::sync::RwLock<VecDeque<RequestLog>>>,
    pub db: Arc<tokio::sync::Mutex<Option<rusqlite::Connection>>>,
    pub http_client: reqwest::Client,
    pub running: Arc<AtomicBool>,
    pub api_key_cipher: Arc<ApiKeyCipher>,   // API Key 加密/解密
    pub auth_token: String,                   // 本地访问令牌（H1 安全修复）
    pub started: Arc<AtomicBool>,            // HTTP 服务惰性启动标记（CAS 保证只启动一次）
    pub last_activity_ms: Arc<AtomicU64>,    // 最近一次服务请求时间戳（空闲自动关闭用）
}
```

---

## 3. 架构设计

### 3.1 模块结构

```
api_hub/
├── mod.rs          # 模块初始化 + 启动入口
├── server.rs       # axum HTTP 服务（路由、Handler、请求处理管线）
├── types.rs        # 数据结构定义
├── provider.rs     # Provider CRUD + 数据库操作
├── commands.rs     # Tauri Command（前端与后端的桥梁）
├── forwarder.rs    # HTTP 转发 + Token 提取 + 日志记录
├── router.rs       # 模型路由 + URL 拼接
├── fetch_models.rs # 从 Provider API 拉取模型列表
├── usage.rs        # 用量统计 + 日志查询
└── transform/
    ├── mod.rs
    ├── anthropic.rs  # OpenAI ↔ Anthropic 格式转换
    ├── responses.rs  # OpenAI Chat ↔ Responses 格式转换
    └── streaming.rs  # Chunk 级 SSE 跨协议流式格式转换
```

### 3.2 请求处理管线

```
第三方应用 (OpenAI Chat 格式)
        │ POST /v1/chat/completions
        ▼
┌─ handle_unified() ──────────────────────────────────────────┐
│                                                              │
│  ① client_request_to_internal()                             │
│     客户端格式 → 内部 OpenAIChat 格式                        │
│     OpenAI Chat   → 直接透传                                 │
│     OpenAI Resp.  → responses_to_chat()                     │
│     Anthropic     → anthropic_to_openai_req()               │
│                                                              │
│  ② internal_request_to_provider()                           │
│     内部 OpenAIChat → Provider 协议格式                      │
│     → OpenAI Chat/Responses: 直接透传                        │
│     → Anthropic:    openai_to_anthropic()                   │
│                                                              │
│  ③ forward_request() / forward_streaming()                  │
│     转发到上游 Provider（认证头 / URL 拼接 / Token 提取）    │
│                                                              │
│  ④ provider_response_to_internal()                          │
│     Provider 响应 → 内部 OpenAIChat 格式                     │
│     → OpenAI Chat/Responses: 直接透传                        │
│     → Anthropic:    anthropic_to_openai()                   │
│                                                              │
│  ⑤ internal_response_to_client()                            │
│     内部 OpenAIChat → 客户端请求的格式                        │
│     OpenAI Chat   → 直接透传                                 │
│     OpenAI Resp.  → chat_to_responses()                     │
│     Anthropic     → openai_response_to_anthropic()          │
│                                                              │
└──────────────────────────────────────────────────────────────┘
        │
        ▼
  JSON Response (客户端请求的协议格式)
```

### 3.3 路由策略

路由模块 (`router.rs`) 根据请求中的 `model` 字段查找对应的 Provider:

1. **精确匹配** — 遍历所有 Provider，匹配 `models` 列表中的模型名（不区分大小写）
2. **通配符匹配** — 按协议前缀匹配（如 `gpt-`/`o1-`/`o3-` 配 OpenAI、`claude-` 配 Anthropic）
3. **无兜底** — 未匹配则返回 `None`，调用方返回 404 明确错误

### 3.4 启动流程（惰性）

```rust
pub fn init(data_dir: &Path) -> AppState {
    // 1. 打开 SQLite 数据库
    // 2. 创建全局 reqwest::Client（连接池复用）
    // 3. 创建共享状态 (providers, request_logs, db, http_client, running, started)
    // 4. 初始化数据库表 + 旧 schema 迁移
    // 5. 从数据库加载已保存的 Provider
}

// 惰性启动：用户首次进入 API Hub 页面时由 api_hub_status 命令触发
pub fn ensure_started(state: &AppState) {
    // CAS 保证只启动一次；未使用 API Hub 的常驻进程不绑定端口、不占后台任务
}

pub async fn start(state: Arc<AppState>) {
    server::start_server(state).await;
    // server::start_server_on 内置空闲自动关闭：
    //   每次 HTTP 请求/前端 Tauri 命令都会刷新 last_activity_ms；
    //   连续 30 分钟无任何调用 → with_graceful_shutdown 优雅退出、
    //   复位 started，下次使用 API Hub 时重新惰性拉起。
}
```

---

## 4. 支持的 Provider

| Provider | 协议 | 上游端点 | 认证方式 | 模型默认前缀 |
|----------|------|----------|----------|-------------|
| OpenAI | `OpenAIChat` | `/v1/chat/completions` | `Authorization: Bearer` | `gpt-`, `o1-`, `o3-`, `o4-`, `text-` |
| OpenAI | `OpenAIResponses` | `/v1/responses` | `Authorization: Bearer` | 同上 |
| Anthropic | `Anthropic` | `/v1/messages` | `x-api-key` + `anthropic-version` | `claude-` |

### 模型自动发现

各 Provider 的模型拉取方式:

- **OpenAI / Responses**: `GET /v1/models`（标准 OpenAI 风格端点，使用 `Authorization: Bearer` 认证）
- **Anthropic**: `GET /v1/models`（使用 `x-api-key` + `anthropic-version` header 认证）

---

## 5. 格式转换映射表

### 5.1 请求参数映射

**OpenAI Chat → Anthropic** (`openai_to_anthropic`):

| OpenAI ChatCompletion | Anthropic Messages | 说明 |
|----------------------|-------------------|------|
| `model` | `model` | 直接透传 |
| `messages[role=system].content` | `system` | 合并多条 system 消息 |
| `messages[role=user].content` | `messages[].role=user` | |
| `messages[role=assistant].content` | `messages[].role=assistant` | |
| `temperature` | `temperature` | 直接透传 |
| `max_tokens` (option) | `max_tokens` | 默认 4096 |
| `stop` | `stop_sequences` | |
| `stream` | `stream` | 直接透传 |
| `top_p` | — | 不支持 |
| `frequency_penalty` | — | 不支持 |
| `presence_penalty` | — | 不支持 |

**Anthropic → OpenAI Chat** (`anthropic_to_openai_req`):

| Anthropic Messages | OpenAI ChatCompletion | 说明 |
|-------------------|----------------------|------|
| `system` | `messages[0].role=system` | 追加到 messages 开头 |
| `messages[].role` | `messages[].role` | 直接透传 |
| `max_tokens` | `max_tokens` | |
| `temperature` | `temperature` | |
| `stop_sequences` | `stop` | |
| `stream` | `stream` | |

**OpenAI Chat → Gemini** — 已删除（不再支持 Gemini 协议）

**Gemini → OpenAI Chat** — 已删除（不再支持 Gemini 协议）

**OpenAI Responses ↔ Chat** (`responses.rs`):

| Responses (`/v1/responses`) | Chat (`/v1/chat/completions`) | 说明 |
|----------------------------|-------------------------------|------|
| `instructions` | `messages[role=system]` | |
| `input[].content` | `messages[].content` | 解析 message 类型的 input |
| `max_output_tokens` | `max_tokens` | |
| `temperature` | `temperature` | 直接透传 |
| `output[].content[].text` | `choices[0].message.content` | |
| `usage.input_tokens` | `usage.prompt_tokens` | |
| `usage.output_tokens` | `usage.completion_tokens` | |

### 5.2 响应 finish_reason 映射

| Provider 原始值 | OpenAI 映射 | Anthropic 映射 | 说明 |
|----------------|-------------|----------------|------|
| STOP / end_turn | `stop` | `end_turn` | 正常结束 |
| MAX_TOKENS / max_tokens | `length` | `max_tokens` | Token 超限 |
| SAFETY / RECITATION | `content_filter` | — | 内容过滤 |
| tool_use | `tool_calls` | `tool_use` | 工具调用 |

### 5.3 Token 提取方案

```rust
pub enum TokenScheme {
    PromptCompletion,  // prompt_tokens + completion_tokens (OpenAI Chat)
    InputOutput,       // input_tokens + output_tokens  (Anthropic / OpenAI Responses)
}
```

---

## 6. SSE 流式处理

流式请求支持 **chunk 级别的跨协议格式转换**：

- **同协议**：直接透传字节流（零开销）
- **跨协议**：通过 `transform/streaming.rs` 逐行解析并转换 SSE 事件

支持的转换方向：

```rust
pub enum StreamDirection {
    OpenAIChatToAnthropic,   // data: {choices:[{delta:{content:"X"}}]} → event: content_block_delta
    AnthropicToOpenAIChat,   // event: content_block_delta → data: {choices:[{delta:{content:"X"}}]}
    OpenAIChatToResponses,   // data: {choices:...} → data: {type:"response.output_text.delta",...}
    ResponsesToOpenAIChat,   // data: {type:"response.output_text.delta",...} → data: {choices:...}
}
```

**处理流程**:

```
客户端 (OpenAI Chat)     API Hub (转换)        上游 Provider (Anthropic)
    │                      │                       │
    │ POST /v1/chat/       │                       │
    │ completions          │                       │
    │ {stream: true}       │                       │
    │─────────────────────►│                       │
    │                      │ POST /v1/messages     │
    │                      │ (stream=true)         │
    │                      │──────────────────────►│
    │                      │                       │
    │                      │  Anthropic SSE events │
    │                      │◄──────────────────────│
    │  OpenAI Chat chunks  │ (chunk级转换)          │
    │◄─────────────────────│                       │
```

**实现细节**:
- 使用 `async_stream` crate 包装转换流
- 缓冲不完整的 SSE 行，按 `\n` 分割后逐行处理
- `StreamState` 跟踪多事件协议转换状态（如 message_start 是否已发出）
- 流式请求超时时间为 60 秒（非流式请求为 30 秒）

---

## 7. Provider 管理 (CRUD)

Provider 通过 Tauri Command 暴露给前端:

| Command | 功能 | 参数 |
|---------|------|------|
| `api_hub_list_providers` | 获取所有 Provider | 无 |
| `api_hub_add_provider` | 添加 Provider | `provider: Provider` |
| `api_hub_delete_provider` | 删除 Provider | `id: String` |
| `api_hub_update_provider` | 更新 Provider | `id: String, provider: Provider` |
| `api_hub_fetch_models` | 从 Provider API 拉取模型 | `base_url, api_key, protocol` |
| `api_hub_status` | 查询服务器状态 | 无 |

### 7.1 数据库表结构

```sql
-- Provider 表
CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    protocol TEXT NOT NULL DEFAULT 'openai_chat',
    base_url TEXT NOT NULL,
    api_key TEXT NOT NULL,
    models TEXT NOT NULL DEFAULT '[]',        -- JSON 数组
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    model_aliases TEXT NOT NULL DEFAULT '{}'  -- JSON 对象
);

-- 请求日志表
CREATE TABLE IF NOT EXISTS request_logs (
    id TEXT PRIMARY KEY,
    provider_id TEXT,
    provider_name TEXT,
    model TEXT,
    request_model TEXT,
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    latency_ms INTEGER DEFAULT 0,
    status_code INTEGER DEFAULT 0,
    error_message TEXT,
    timestamp INTEGER NOT NULL,
    is_streaming INTEGER DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON request_logs(timestamp);
```

### 7.2 Schema 迁移

支持从旧版本（`provider_type` + `api_format` 两列）到单一 `protocol` 列的平滑迁移：

```sql
-- 合并逻辑
UPDATE providers SET protocol = CASE
    WHEN provider_type = 'anthropic' THEN 'anthropic'
    WHEN provider_type = 'openai' AND api_format = 'responses' THEN 'openai_responses'
    ELSE 'openai_chat'
END;
```

注意：已不再支持 Gemini 和 Ollama 协议。旧数据库中配置为这两种协议的 Provider 记录将在迁移时被删除。

---

## 8. 用量统计与请求日志

### 8.1 日志记录

所有请求（成功和失败）均记录到内存和 SQLite:

- 内存中保留最近 **1000 条**日志（环形缓冲）
- SQLite 持久化全部日志（含 `timestamp` 索引）

### 8.2 统计数据

| Command | 功能 |
|---------|------|
| `api_hub_get_logs` | 获取请求日志（支持 limit/offset 分页） |
| `api_hub_get_usage_stats` | 获取用量统计 |

**统计数据包含**:

```rust
pub struct UsageStats {
    pub total_requests: u64,        // 总请求数
    pub total_errors: u64,          // 总错误数
    pub total_input_tokens: u64,    // 总输入 token
    pub total_output_tokens: u64,   // 总输出 token
    pub total_latency_ms: u64,      // 总延迟
    pub avg_latency_ms: u64,        // 平均延迟
    pub by_model: HashMap<String, ModelStats>,   // 按模型聚合
    pub by_hour: HashMap<i64, HourlyStats>,      // 按小时聚合（最近 24h）
}
```

### 8.3 按小时聚合

日志统计中，最近 24 小时的请求按时段（整点）聚合:

```rust
let secs_ago = now - log.timestamp;
if secs_ago < 86400 {  // 24 小时内
    let hour_key = (log.timestamp / 3600) * 3600;
    // 累加到对应 hour 的统计中
}
```

---

## 9. 安全说明

### 9.1 监听地址

API Hub 的 HTTP 服务仅监听 **localhost** (`127.0.0.1:3456`)，默认不对外暴露:

```rust
pub async fn start_server(state: Arc<AppState>) {
    start_server_on(state, "127.0.0.1:3456").await;
}
```

### 9.2 安全设计

| 设计 | 说明 |
|------|------|
| 仅监听 localhost | 外部网络无法访问，天然隔离 |
| 请求体大小限制 | `DefaultBodyLimit::max(10MB)` 防止内存耗尽攻击 |
| 本地访问令牌鉴权 | 代理端点（`/v1/*`）要求携带 `X-DevNexus-Token` 或 `Authorization: Bearer <token>`，令牌在启动时生成于 `AppState.auth_token` 并下发给前端；用于防止本机其他进程 / 恶意网页（DNS rebinding、CSRF 式请求）盗用已配置 Provider 的 API Key |
| API Key 加密存储 | API Key 通过 `ApiKeyCipher`（`api_hub/crypto.rs`）加密后以密文存入本地 SQLite（`providers` 表 `api_key` 字段不再明文） |
| CORS | 仅允许 `tauri://localhost` 与本地 dev 地址（`http://localhost:1420` 等）跨域调用 |

### 9.3 已知风险

- **端口冲突**: `3456` 端口被占用时启动失败（日志打印错误信息）
- **令牌下发**: 前端需从 `api_hub_status` 获取 `auth_token` 并在请求代理端点时携带，否则返回 401

---

## 10. 第三方工具集成

任何指向 `http://localhost:3456/v1` 的工具均可使用 API Hub 中的所有模型。

### 10.1 Cursor 配置

```
Settings → Models → OpenAI API Key: (任意值)
OpenAI Base URL: http://localhost:3456/v1
```

### 10.2 VS Code (Continue 插件)

```json
{
  "models": [{
    "title": "DevNexus API Hub",
    "provider": "openai",
    "apiBase": "http://localhost:3456/v1",
    "apiKey": "ignored"
  }]
}
```

### 10.3 curl 命令行

```bash
# OpenAI 格式
curl http://localhost:3456/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-sonnet-4","messages":[{"role":"user","content":"Hello"}],"stream":true}'

# Anthropic 格式
curl http://localhost:3456/v1/messages \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}],"max_tokens":1024}'

# 健康检查
curl http://localhost:3456/health

# 列出所有可用模型
curl http://localhost:3456/v1/models
```

### 10.4 SDK 集成

```python
# OpenAI Python SDK
from openai import OpenAI
client = OpenAI(
    base_url="http://localhost:3456/v1",
    api_key="not-needed"
)
response = client.chat.completions.create(
    model="claude-sonnet-4",
    messages=[{"role": "user", "content": "Hello"}]
)
```

---

## 11. URL 拼接策略

路由模块中的 `build_upstream_url` 函数负责拼接 Provider 的 `base_url` 与协议 `endpoint`:

```rust
pub fn build_upstream_url(provider: &Provider, endpoint: &str) -> String {
    let base = provider.base_url.trim_end_matches('/');
    join_path(base, endpoint)
}
```

**去重逻辑** `join_path` 自动处理路径重叠（例如 base 以 `/v1` 结尾，endpoint 以 `/v1` 开头 → 只保留一份 `/v1`）:

```
join_path("https://gy.hetaosu.xyz/v1", "/v1/chat/completions")
  → "https://gy.hetaosu.xyz/v1/chat/completions"
```

---

## 12. 测试

```rust
// router 测试
#[test] fn test_join_path_normal()
#[test] fn test_join_path_double_v1()
#[test] fn test_join_path_trailing_slash()
#[test] fn test_join_path_no_overlap()
#[test] fn test_join_path_base_has_no_v1()

// anthropic 转换测试
#[test] fn test_system_message_extraction()
#[test] fn test_openai_response_to_anthropic()

// streaming 转换测试
#[test] fn test_openai_to_anthropic_content_delta()
#[test] fn test_anthropic_to_openai_content_delta()
#[test] fn test_openai_to_responses_delta()
#[test] fn test_done_signal()

// responses 转换测试
#[test] fn test_responses_to_chat_basic()
#[test] fn test_chat_request_to_responses()
#[test] fn test_chat_to_responses_basic()

// E2E 测试
#[tokio::test] fn e2e_health_and_models()
#[tokio::test] fn e2e_openai_to_openai_passthrough()
#[tokio::test] fn e2e_openai_client_to_anthropic_upstream()
#[tokio::test] fn e2e_anthropic_client_to_openai_upstream()
#[tokio::test] fn e2e_openai_client_to_responses_upstream()
#[tokio::test] fn e2e_missing_model_returns_400()
#[tokio::test] fn e2e_streaming_chat()
#[tokio::test] fn e2e_streaming_cross_protocol_openai_to_anthropic()
#[tokio::test] fn e2e_unmatched_model_returns_404()
```

测试覆盖: URL 拼接去重、各协议格式转换的正确性、chunk 级流式跨协议转换、system message 提取、Token 字段映射、未匹配模型 404。
