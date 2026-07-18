# DevNexus API Hub — Context

## Goal
在 DevNexus 中提供一个统一的 API 聚合网关，将多个 AI 模型服务（OpenAI、Anthropic、Google Gemini、Ollama）的 API 统一为一个本地端口，并实现各格式之间的双向转换。让所有第三方应用（VS Code、Cursor、CLI 工具等）只需配置一个地址就能访问所有模型。

## User
- DevNexus 用户（开发者）
- 日常使用多种 AI 模型服务，需要在多个 API Key 和不同 API 格式之间切换
- 技术能力较强，熟悉 API 调用和模型配置

## JTBD (Job To Be Done)
1. 配置一个 Provider（添加 API Key / 本地模型地址）
2. 在一个统一页面上查看所有可用模型及其状态
3. 第三方应用指向 `http://localhost:{port}/v1` 即可使用所有模型
4. 调试：查看请求日志、测试模型连通性

## Constraints
- **Stack**: Rust (Tauri backend, axum HTTP), Svelte (frontend)
- **Existing deps**: tokio, reqwest, serde — 已可用
- **App model**: Tauri 常驻后台，支持托盘图标
- **Port**: 避免与常见端口冲突

## Supported Providers (v1)
| Provider | API Endpoints | 格式 |
|----------|--------------|------|
| OpenAI | `/chat/completions`, `/responses` | OpenAI |
| Anthropic | `/v1/messages` | Anthropic |
| Google Gemini | 待定 | Google |
| Ollama | 本地模型 | Ollama / OpenAI 兼容 |

## Format Conversion (双向)
- OpenAI `chat/completions` ↔ Anthropic `messages`
- OpenAI `chat/completions` ↔ Google Gemini
- OpenAI `chat/completions` ↔ Ollama
- Streaming (SSE) 透传 + 格式转换

## Success Criteria
- 启动后第三方应用（如 Cursor）配置 `localhost:{port}` 即可列出所有已配置的 Provider 的模型
- 通过任意支持的格式调用任意后端的模型，都能正确返回
- 流式输出正常工作
- 页面清晰显示各 Provider 状态、延迟、请求计数

## Scope v1
- [x] 后台 HTTP 服务（axum）作为 API 网关
- [x] 独立功能页面（Provider 管理 + 模型列表 + 请求日志）
- [x] Provider CRUD（添加/编辑/删除 API Key 和端点）
- [x] OpenAI ↔ Anthropic 格式互转
- [x] OpenAI ↔ Ollama 格式互转
- [x] Streaming SSE 支持
- [x] CORS 支持（第三方应用跨域访问）

## Non-goals (v1)
- [ ] 负载均衡 / 多 Key 轮询
- [ ] 用量计费 / Token 统计
- [ ] 请求缓存
- [ ] 访问控制 / 鉴权（v1 仅监听 localhost）

## Open Assumptions
- 用户在本机使用，无需身份验证
- 用户至少有其中一个 Provider 的 API Key
- Ollama 用户已在本机安装 Ollama

## ⚠ Risks
- 不同 Provider 的参数映射存在精度损失（如 system prompt vs messages.role）
- Streaming 格式转换需逐 chunk 处理，可能引入延迟
- Anthropic 的 `/v1/messages` 流式格式与 OpenAI 差异较大
