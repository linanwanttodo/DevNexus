# API Hub — 最终实施方案

## 选定方案：A — Unified Dashboard (with enhancements)

结合 cc-switch 的架构参考和 ZCode 的图表风格，增强方案 A。

## 页面布局

```
┌────────────────────────────────────────────────────────────┐
│  API Hub                                        ● :3456   │
│  Status: Running                    ↑ 128 req today       │
├────────────────────────────────────────────────────────────┤
│  [ Overview ] [ Providers ] [ Logs ]                        │
├────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──── Overview Tab ────────────────────────────────────┐  │
│  │                                                       │  │
│  │  ┌─ Stats Cards ──────────────────────────────────┐  │  │
│  │  │  Total Reqs   Active Models   Error Rate   Avg  │  │  │
│  │  │  1,284        12              0.8%         1.2s │  │  │
│  │  └────────────────────────────────────────────────┘  │  │
│  │                                                       │  │
│  │  ┌─ Token Usage Trend (面积图) ───────────────────┐  │  │
│  │  │  ▁▂▃▅▇▆▄▃▂▁▃▅▇▆▅▃▂▁▂▄▆▇▆▅▃▂               │  │  │
│  │  │  ● Input Tokens  ● Output Tokens  ● Cost      │  │  │
│  │  └────────────────────────────────────────────────┘  │  │
│  │                                                       │  │
│  │  ┌─ Models Bar Chart ────────────────────────────┐  │  │
│  │  │  gpt-4o        ████████████▌  1,284 req       │  │  │
│  │  │  claude-sonnet ███████▋       824 req         │  │  │
│  │  │  llama3.2      ████▊          512 req         │  │  │
│  │  │  gpt-4o-mini   ███▍           384 req         │  │  │
│  │  └────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌──── Providers Tab ───────────────────────────────────┐  │
│  │  ┌──────────────────────────────────────────────────┐ │  │
│  │  │  [+ Add Provider]                       [Refresh]│ │  │
│  │  ├──────────────────────────────────────────────────┤ │  │
│  │  │  OpenAI          ● Active      8 models  ~32ms  │ │  │
│  │  │  └─ gpt-4o, gpt-4o-mini, ...                    │ │  │
│  │  ├──────────────────────────────────────────────────┤ │  │
│  │  │  Anthropic       ● Active      6 models  ~45ms  │ │  │
│  │  │  └─ claude-sonnet-4, claude-haiku-3, ...        │ │  │
│  │  ├──────────────────────────────────────────────────┤ │  │
│  │  │  Ollama          ● Online      3 models  ~2ms   │ │  │
│  │  │  └─ llama3.2, qwen2.5, nomic-embed-text        │ │  │
│  │  └──────────────────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌──── Logs Tab ────────────────────────────────────────┐  │
│  │  [Filter: All ▼] [Model: ▼] [Status: ▼]  [Search]  │  │
│  │  ┌──────────────────────────────────────────────────┐│  │
│  │  │ Time      Model       Tokens   Status  Latency  ││  │
│  │  │ 14:32:15  gpt-4o      ↑128↓64  ✓ 200   1.2s    ││  │
│  │  │ 14:32:10  claude-s      ↑256↓32  ✓ 200   2.1s  ││  │
│  │  │ 14:31:55  llama3.2     ↑64↓128  ✓ 200   0.8s   ││  │
│  │  └──────────────────────────────────────────────────┘│  │
│  └───────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

## 架构设计（参考 cc-switch）

### Rust 后端

```
src-tauri/src/
├── api_hub/
│   ├── mod.rs              # 模块入口
│   ├── server.rs           # axum HTTP 服务器（localhost:3456）
│   ├── router.rs           # 请求路由（/v1/chat/completions, /v1/messages 等）
│   ├── provider.rs         # Provider 数据结构管理
│   ├── types.rs            # 共享类型定义
│   │
│   ├── transform/          # API 格式转换
│   │   ├── mod.rs
│   │   ├── openai.rs       # OpenAI 格式解析/生成
│   │   ├── anthropic.rs    # Anthropic 格式解析/生成
│   │   ├── gemini.rs       # Google Gemini 格式解析/生成
│   │   └── ollama.rs       # Ollama 格式处理
│   │
│   ├── streaming/          # SSE 流式处理
│   │   ├── mod.rs
│   │   ├── openai_sse.rs   # OpenAI 流式格式
│   │   └── anthropic_sse.rs# Anthropic SSE 格式
│   │
│   ├── models.rs           # 模型映射和管理
│   ├── health.rs           # 健康检查
│   └── usage/              # 用量统计
│       ├── mod.rs
│       ├── tracker.rs       # 请求计数和 Token 统计
│       └── store.rs         # 持久化存储（SQLite）
```

### 核心转换流程

```
Client Request
     │
     ▼
axum Router (/v1/chat/completions)
     │
     ▼
Provider Router ───→ Model Mapper ───→ Format Transform
     │                                        │
     │                              ┌─────────┴──────────┐
     │                              │   OpenAI → Claude   │
     │                              │   Claude → OpenAI   │
     │                              │   OpenAI → Gemini   │
     │                              │   OpenAI → Ollama   │
     │                              └─────────┬──────────┘
     │                                        │
     ▼                                        ▼
Forwarder ─────────────────────────→ Upstream API
     │                                        │
     │                              ┌─────────┴──────────┐
     │                              │  Response Transform │
     │                              │  (reverse mapping)  │
     │                              └─────────┬──────────┘
     ▼                                        ▼
Client Response ←──── SSE Streaming ←─── Formatted Response
```

### 格式转换映射表

| 输入格式 → 后端 | chat/completions | messages | generateContent |
|----------------|-----------------|----------|-----------------|
| **OpenAI**     | 透传            | 转 Anthropic | 转 Gemini |
| **Anthropic**  | 转 OpenAI       | 透传       | 转 Gemini |
| **Gemini**     | 转 OpenAI       | 转 Anthropic | 透传 |
| **Ollama**     | 透传(OpenAI兼容) | 转 OpenAI → Anthropic | 转 OpenAI → Gemini |

### Key 参数映射

| 参数 | OpenAI | Anthropic | Gemini | Ollama |
|------|--------|-----------|--------|--------|
| model | `model` | `model` | `model` | `model` |
| messages | `messages[{role,content}]` | `messages`+`system` | `contents[{role,parts}]` | `messages`(同OAI) |
| system | 在messages中role=system | `system`字段 | `system_instruction` | 在messages中 |
| temperature | `temperature` | `temperature` | `generationConfig.temperature` | `temperature` |
| max_tokens | `max_tokens` | `max_tokens` | `generationConfig.maxOutputTokens` | `max_tokens` |
| stream | `stream: true` | `stream: true` | `stream: true` | `stream: true` |
| stop | `stop` | `stop_sequences` | `generationConfig.stopSequences` | `stop` |

### 前端图表库

使用 **recharts**（与 cc-switch 一致），因为：
- React 生态最成熟的图表库，Svelte 可通过 wrapper 使用
- 或者用纯 SVG/CSS 实现轻量化图表（无需额外依赖）

**Token 面积图（UsageTrendChart 风格）**：
- recharts `AreaChart` 显示输入/输出 Token 趋势
- 时间范围选择（1h / 6h / 24h / 7d）
- 自定义 Tooltip 显示详细数据

**模型请求柱状图**：
- recharts `BarChart` 显示各模型请求量
- 横向柱状图，按请求数排序

**每日热力图**：
- 24h × 7d 的热力图网格
- 颜色深浅表示请求密度
- 纯 CSS Grid 实现，无需额外图表库

## 数据持久化

使用 SQLite（已有 `rusqlite` 依赖）：
- `providers` 表 — Provider 配置
- `usage_logs` 表 — 请求日志和 Token 统计
- 数据存储在 `data_dir()/api_hub/` 下

## 实施计划

| 步骤 | 内容 | 预估 |
|------|------|------|
| 1 | Rust: axum server + 基础路由 + CORS | 1天 |
| 2 | Rust: Provider 管理（CRUD + 持久化） | 1天 |
| 3 | Rust: OpenAI ↔ Anthropic 格式互转 | 2天 |
| 4 | Rust: OpenAI ↔ Gemini/Ollama 格式互转 | 1天 |
| 5 | Rust: SSE 流式转换 + 透传 | 1天 |
| 6 | Rust: 用量统计（tracker + store） | 1天 |
| 7 | Svelte: API Hub 功能页面（3个Tab） | 2天 |
| 8 | Svelte: Token 面积图 + 柱状图 + 热力图 | 1天 |
| 9 | 集成测试 + 调试 | 1天 |
| | **总计** | **~11天** |
