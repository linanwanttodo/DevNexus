# DevNexus SSH 模块设计

> 日期：2026-08-16
> 状态：已确认
> 范围：连接管理 + 交互式终端 + SFTP（核心版），通用侧边栏导航上下文机制

## 1. 背景与目标

DevNexus 目前是 Tauri 2 + Vue 3 + Rust 的跨平台开发者工具箱，侧边栏为 13 个模块的扁平列表，无任何 SSH 能力。本次新增：

1. **SSH 模块**：连接配置管理 + 交互式终端 + SFTP 文件管理（对标 Termius/Xshell/MaidKit 核心能力）
2. **通用导航上下文机制**：侧边栏改为"图标轨 + 上下文面板"双栏，点击带上下文的模块（如 SSH）后右侧面板展示该模块的子导航项；其他模块未来可复用该机制

## 2. 技术决策

| 决策点 | 选择 | 理由 |
|---|---|---|
| SSH 库 | `russh` + `russh-sftp` | 纯 Rust 异步，契合项目 tokio 栈，无 C 依赖，支持 PTY/SFTP/私钥认证 |
| 终端渲染 | `@xterm/xterm` (xterm.js) | Webview 内标准终端模拟器，与 PTY 流式配合 |
| 凭据存储 | AES-256-GCM + PBKDF2 + OS keyring | 复用现有密码管理器加密方案，抽共享 `crypto.rs` |
| 认证 | 密码 / 私钥（可带口令） | 覆盖主流服务器场景 |
| 终端会话 | 多会话标签页 | 主流 SSH 客户端基本体验 |
| SFTP | 基础文件操作 + 拖拽传输 | 覆盖日常文件管理 |

## 3. 整体架构

```
┌────────────────────────────── Vue 3 前端 ──────────────────────────────┐
│ 侧边栏 (双栏)：图标轨 + 上下文面板  →  导航上下文机制 (通用)              │
│   SSH 上下文面板：连接列表 / 终端会话 / SFTP                            │
│ 页面：SSHConnections · SSHTerminal · SSHSftp                           │
└────────────────────────────────────────────────────────────────────────┘
                          │  invoke / emit (Tauri IPC)
┌────────────────────────────── Rust 后端 ───────────────────────────────┐
│ commands/ssh/                                                          │
│   mod.rs          — 命令注册 + 事件路由                                │
│   connections.rs  — 连接配置 CRUD（加密凭据）                          │
│   session.rs      — SSH 连接池（russh），每连接一个后台任务             │
│   terminal.rs     — PTY 会话管理：request_pty + shell + 数据流桥接     │
│   sftp.rs         — russh-sftp 封装                                    │
│ utils/crypto.rs   — 共享 AES-256-GCM 加解密（从 password_manager 抽出）│
└────────────────────────────────────────────────────────────────────────┘
```

### 模块边界

| 单元 | 职责 | 依赖 |
|---|---|---|
| `connections.rs` | 连接配置 CRUD、凭据加密存储、连接测试 | `crypto.rs`, 配置文件 |
| `session.rs` | 连接池维护、认证、`known_hosts` 校验 | `russh` |
| `terminal.rs` | PTY 通道生命周期、输入输出双向流 | `session.rs`, Tauri 事件 |
| `sftp.rs` | SFTP 目录/文件操作、分块传输 + 进度 | `session.rs`, `russh-sftp` |
| 前端 `nav-config.js` | 导航配置单一事实来源（主模块 + 可选 context） | 无 |
| 前端 `Sidebar.vue` | 双栏渲染 + 上下文面板联动 | `nav-config.js`, 路由 |

## 4. 后端设计

### 4.1 连接配置 (`connections.rs`)

```rust
struct SshConnection {
    id: String,                 // uuid
    name: String,
    host: String,
    port: u16,                  // 默认 22
    username: String,
    auth_type: AuthType,        // Password | PrivateKey
    encrypted_secret: String,   // 加密后的密码 或 私钥内容
    key_passphrase_encrypted: Option<String>,  // 私钥口令（加密）
    created_at: i64,
    updated_at: i64,
}

enum AuthType { Password, PrivateKey }
```

- 存储：JSON 文件（`~/.config/devnexus/ssh_connections.json`，与 password_manager 的 entries 模式一致）
- 加密：AES-256-GCM，密钥 32B 存 OS keyring（`keyring` crate 已依赖），无 keyring 时退回本地派生
- 命令：
  - `ssh_list_connections() -> Vec<SshConnectionInfo>`（不含密文）
  - `ssh_save_connection(conn) -> SshConnectionInfo`
  - `ssh_delete_connection(id)`
  - `ssh_test_connection(id)` → 尝试认证返回结果
- **安全**：明文密码/私钥只在 Rust 侧认证时解密，永不回传前端

### 4.2 连接池与会话 (`session.rs`)

```rust
struct SshSessionManager {
    sessions: Mutex<HashMap<String, SessionEntry>>,  // session_id -> entry
    known_hosts: Mutex<HashMap<String, Fingerprint>>, // host -> 指纹
}
struct SessionEntry {
    handle: russh::client::Handle<SshHandler>,
    connection_id: String,
}
```

- 生命周期：`open`（连接+认证）→ 复用；空闲超时回收
- Host key：首次连接 emit `ssh-hostkey-prompt`，前端 ConfirmDialog 展示指纹；确认后写入 `known_hosts.json`，后续自动比对
- 命令：`ssh_open(connectionId) -> sessionId`、`ssh_close(sessionId)`
- Handler 实现 `russh::client::Handler` trait：`check_server_key`（指纹校验）、`channel_open_confirmation`、事件回传

### 4.3 终端会话 (`terminal.rs`)

流程：
1. `ssh_terminal_open(connectionId, cols, rows)` → 复用/新建连接 → `channel_open_session` → `request_pty("xterm-256color", cols, rows, ...)` → `shell()`
2. 后台 task 循环 `ChannelMsg::Data`/`ExtendedData`/`Close` → `emit("ssh-terminal-output", { sessionId, data })`
3. `ssh_terminal_input(sessionId, data)` → `channel.data(data)`
4. `ssh_terminal_resize(sessionId, cols, rows)` → `request_pty_size` / `window_change`
5. `ssh_terminal_close(sessionId)` → `channel_close` + 清理

### 4.4 SFTP (`sftp.rs`)

基于 `russh-sftp`（v3）：
- `ssh_sftp_open(connectionId) -> sftpSessionId`（复用连接池 handle，`channel_open_subsystem("sftp")`）
- `ssh_sftp_list_dir(sftpSessionId, path)` → 条目列表（name/type/size/mode/mtime）
- `ssh_sftp_read_file(sessionId, remotePath, offset, length)` → 分块下载，前端拼接；大文件进度 `emit("ssh-sftp-progress")`
- `ssh_sftp_write_file(sessionId, remotePath, data)` → 分块上传
- `ssh_sftp_rename` / `ssh_sftp_delete` / `ssh_sftp_mkdir`
- 拖拽传输：前端封装拖拽 → 调用上述命令，进度条驱动

## 5. 前端设计

### 5.1 通用导航上下文机制

新增 `src/lib/nav-config.js`：

```js
export const navItems = [
  { id: "dashboard", route: "/dashboard", icon: "dashboard", labelKey: "nav.dashboard" },
  // ... 现有模块（无 context）
  { id: "ssh", route: "/ssh", icon: "server", labelKey: "nav.ssh",
    context: {
      titleKey: "nav.ssh",
      items: [
        { route: "/ssh", icon: "list", labelKey: "ssh.connections" },
        { route: "/ssh/sessions", icon: "terminal", labelKey: "ssh.sessions" },
        { route: "/ssh/sftp", icon: "folder", labelKey: "ssh.sftp" },
      ],
    } },
];
```

`Sidebar.vue` 改造：
- 左侧图标轨：渲染所有主模块（宽度收窄）
- 右侧上下文面板：`activeNav.context` 存在时渲染其 items，不存在则收起
- `selectedKey` 推导：`route.path` 前缀匹配主模块（如 `/ssh/sessions` → `ssh`）
- 点击主模块 → 若带 context 且当前非该模块 → 面板展开并导航到该模块默认路由

`router.js` 新增：
- `/ssh` → SSHConnections.vue
- `/ssh/sessions` → SSHTerminal.vue
- `/ssh/sftp` → SSHSftp.vue

### 5.2 SSH 视图

| 视图 | 路由 | 内容 |
|---|---|---|
| SSHConnections.vue | `/ssh` | 连接卡片/列表、增删改表单、连接测试、双击打开终端 |
| SSHTerminal.vue | `/ssh/sessions` | 多标签终端：xterm.js + 标签栏 + 输入/输出/缩放/重连 |
| SSHSftp.vue | `/ssh/sftp` | 双栏文件浏览器 + 拖拽传输 + 进度条 |

- xterm.js：`@xterm/xterm` + `@xterm/addon-fit`，懒加载（配合懒路由）
- 新建连接默认跳转 `/ssh/sessions` 打开终端

### 5.3 i18n

`zh.json`/`en.json`/`ru.json` 新增 `nav.ssh`、`ssh.*`（connections/sessions/sftp/add/edit/delete/test/…）键。

## 6. 数据流 / 错误处理 / 测试

### 数据流
- 终端输出：`ChannelMsg::Data` → `emit("ssh-terminal-output")` → xterm.js `write()`
- 终端输入：xterm.js `onData` → `invoke("ssh_terminal_input")` → `channel.data()`
- 断线：`ChannelMsg::Close`/错误 → `emit("ssh-session-closed")` → 前端标记 tab 支持重连

### 错误处理
- 结构化错误码：`AuthFailed` / `Timeout` / `HostKeyMismatch` / `KeyringUnavailable` / `SftpError`
- 认证失败、连接超时 → toast 提示
- 凭据解密失败 → 明确报错，提示 keyring 状态

### 测试
- Rust 单测：`crypto.rs` 加解密 roundtrip（复用现有模式）、`connections.rs` CRUD 序列化、`session.rs` 指纹比对逻辑
- 前端单测：`nav-config` 路由 → 主模块推导逻辑
- 手动集成：连测试服务器验证 终端 + SFTP 全流程、断线重连、拖拽传输

## 7. 不在本期范围（后续扩展）

- 服务器性能监控/图表、进程/服务/防火墙管理、端口转发、容器管理（对标 MaidKit 其余功能）
- 会话持久化（终端滚动缓冲落盘）
- 隧道/代理

## 8. 依赖新增

Rust (`src-tauri/Cargo.toml`)：
- `russh = "0.62"`（最新稳定版 0.62.6）
- `russh-sftp = "2.4"`（最新稳定版 2.4.0，russh 配套版本）

前端 (`package.json`)：
- `@xterm/xterm = "6.x"`（最新 6.0.0）
- `@xterm/addon-fit`（配套版本）