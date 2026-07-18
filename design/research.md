# API Hub — Design Research

## Sources
- One API (songquanpeng/one-api) — 开源统一 API 网关
- LiteLLM (BerriAI/litellm) — LLM 代理 + 管理后台
- Portkey — AI Gateway (企业级)
- OpenRouter — 统一模型访问入口

## Conventions (users expect)

### Provider/Channel 管理
- **表格布局**：Provider 列表以表格展示，每行一个渠道，包含：名称、类型（OpenAI/Anthropic等）、状态（启用/禁用）、模型列表、操作（编辑/删除/测试）
- **表单编辑**：添加/编辑使用模态对话框或独立页面，填写 Base URL、API Key、模型映射
- **状态指示**：绿色圆点=在线，红色=异常，灰色=禁用
- **优先级排序**：支持拖拽或数字排序，决定请求路由优先级

### API Key / 令牌管理
- **虚拟 Key**：生成用于第三方应用的 Key，可限制模型访问范围和额度
- **Key 列表**：表格展示 Key 名称、前缀、状态、最后使用时间
- **创建弹窗**：模态框创建，支持设置别名、过期时间、模型白名单

### 模型列表
- **分 Provider 分组**：按 Provider 分组展示可用模型
- **状态标签**：在线/离线/速率限制
- **搜索过滤**：快速定位模型

### 请求日志
- **实时日志流**：类似终端输出或表格刷新
- **表格视图**：时间、模型、请求耗时、Token 数、状态码、来源 IP
- **筛选器**：按 Provider、模型、状态码、时间范围过滤
- **详情查看**：点击展开查看完整请求/响应体

## Pains (competitors leave unsolved)

| Pain | In which product | Opportunity |
|------|-----------------|-------------|
| 配置复杂，需要 YAML 文件编辑 | LiteLLM | 全 GUI 配置，零配置文件 |
| 格式转换靠文档摸索 | One API | 自动格式转换 + 可视化映射 |
| 本地模型(Ollama)接入步骤多 | 通用痛点 | 一键检测本地 Ollama |
| API 测试需要 curl 或 Postman | OpenRouter | 内置 Playground 直接发请求测试 |
| 日志不够直观 | One API | 实时流日志 + 可视化耗时分析 |

## Gaps to exploit
- **零配置起步**：自动检测本地运行中的 Ollama 实例，一键启用
- **格式转换可视化**：在界面上展示"当前请求从 OpenAI 格式 → Anthropic"的实时转换过程
- **Tauri 集成优势**：系统托盘快捷开关、全局快捷键调出
- **DevNexus 生态联动**：与 Password Manager 集成存储 API Key

## Layout Patterns (synthesized)

### 页面结构（左导航 + 主内容）
```
┌──────────┬────────────────────────────────────────┐
│ Sidebar  │  Header (Provider 状态总览)              │
│          ├────────────────────────────────────────┤
│ Overview │  Tab Bar:                              │
│ Providers│  [ Providers | Models | Keys | Logs ]  │
│ Models   │                                        │
│ Keys     │  Content Area (tab-dependent)          │
│ Logs     │                                        │
│          │                                        │
└──────────┴────────────────────────────────────────┘
```

### Provider Tab 的布局
```
┌─────────────────────────────────────────────┐
│ [+ Add Provider]                 [Refresh]  │
├─────────────────────────────────────────────┤
│ ┌─────────────────────────────────────────┐ │
│ │ OpenAI              ● Active    [Edit]  │ │
│ │ gpt-4o, gpt-4o-mini   ↓32ms            │ │
│ │─────────────────────────────────────────│ │
│ │ Anthropic           ● Active    [Edit]  │ │
│ │ claude-sonnet-4, ...  ↓45ms            │ │
│ │─────────────────────────────────────────│ │
│ │ Ollama (local)      ● Detected  [Edit]  │ │
│ │ llama3.2, qwen2.5    ↓2ms              │ │
│ └─────────────────────────────────────────┘ │
└─────────────────────────────────────────────┘
```

### 日志 Tab 的布局
```
┌─────────────────────────────────────────────┐
│ [Filter: All ▼] [Model: All ▼] [Status ▼]  │
├─────────────────────────────────────────────┤
│ Time         Model      Status  Latency  🡕 │
│ 14:32:15  gpt-4o       ✓ 200    1.2s    ▶  │
│ 14:32:10  claude-sonnet ✓ 200    2.1s    ▶  │
│ 14:31:55  llama3.2     ✓ 200    0.8s    ▶  │
│ 14:31:50  gpt-4o       ✗ 429    0.1s    ▶  │
└─────────────────────────────────────────────┘
```
