# SSH — 模块设计文档

> 架构设计见 `docs/superpowers/specs/2026-08-16-ssh-module-design.md`，实现计划见
> `docs/superpowers/plans/2026-08-16-ssh-module.md`。

## 1. 功能概述

SSH 模块为 DevNexus 提供对标 Termius / Xshell 的核心能力：连接配置管理、交互式多标签
终端、SFTP 文件管理，并在此之上扩展了跳板机（ProxyJump）、本地端口转发（-L）、动态
SOCKS5 代理（-D）、SSH Agent 转发、OpenSSH config 导入/导出，以及复用 API Hub LLM
配置的终端/SFTP AI 助手。

- 后端基于 `russh` 0.62（纯 Rust 异步，契合 tokio 栈）+ `russh-sftp` 2.4
- 终端渲染使用 `@xterm/xterm` + `@xterm/addon-fit`（懒加载，配合懒路由）
- 凭据 AES-256-GCM 加密（密钥存 OS keyring，复用共享 `CryptoVault`），明文只在 Rust 侧认证
- 终端输入输出与文件内容一律 base64 传输（避免任意字节的 UTF-8 损失）

---

## 2. 后端结构

```
src-tauri/src/commands/ssh/
  mod.rs          — 模块导出
  connections.rs  — 连接配置 CRUD（加密存储 shadow，含 group/tags/keepalive/jump_host）
  session.rs      — 连接池、认证、host key 校验、端口转发/SOCKS/Agent、Keepalive
  terminal.rs     — PTY 终端会话、输入输出数据流、环形输出缓冲（AI 读屏）
  sftp.rs         — SFTP 目录列举、分块读写、增删改
  ai.rs           — 复用 API Hub Provider 的 LLM 助手（终端 + SFTP）
```

### 2.1 连接配置（connections.rs）

存储 `ssh_connections.json`，字段含：`id/name/host/port/username/auth_type/
encrypted_secret/key_passphrase_encrypted/created_at/updated_at/group/tags/
last_connected/keepalive_secs/jump_host_id`。所有凭据字段均为密文，回传前端只含
`SshConnectionInfo`（脱敏）。

- `ssh_list_connections` — 列表（无密文）
- `ssh_save_connection` — 新增/更新
- `ssh_delete_connection` — 删除
- `ssh_touch_connection` — 记录最近连接时间
- `ssh_import_open_ssh_config` / `ssh_export_openssh_config` — OpenSSH config 导入/导出

`SshStore` 采用**懒加载**：`new()` 不预读文件，首次命令访问时 `ensure_loaded()` 再读，
配合 `ssh_touch_connection` 由前端在连接页访问时确保已加载。

### 2.2 会话与认证（session.rs）

- `open(conn_id)` → `session_id`，流程：TCP 连接 → SSH 握手 → host key 校验 → 认证
  （密码 / 私钥+口令）→ 入连接池
- host key 首连：emit `ssh-hostkey-prompt`，前端确认后 `ssh_hostkey_accept`/`reject`，
  落盘 `known_hosts.json`；指纹变更则拒绝
- 跳板机（ProxyJump）：`open_via_jump` 先连跳板、认证后开 direct-tcpip 通道到目标，
  再在通道之上跑目标 SSH 握手
- 端口转发（-L）：`ssh_forward_local` 本地监听 → direct-tcpip 转发；`ssh_close_forward`/`ssh_list_forwards`
- 动态 SOCKS5（-D）：`ssh_socks_proxy` 本地监听实现 SOCKS5 握手（无认证/CONNECT，
  支持 IPv4/域名/IPv6）→ 经隧道转发
- Agent 转发：`ssh_forward_agent` 校验 `SSH_AUTH_SOCK` 存在性
- `SessionEntry` 各通道句柄 `Arc` 化，远端 I/O 不占用会话表锁

### 2.3 终端（terminal.rs）

- `ssh_terminal_open(conn_id, cols, rows)` → PTY + shell
- 读循环 `ChannelMsg::Data/ExtendedData` → emit `ssh-terminal-output`（base64），同时
  写入 `TerminalBuffer`（2000 行环形缓冲，供 AI 读屏）
- `ssh_terminal_input`（base64）/ `ssh_terminal_resize` / `ssh_terminal_close`
- 断线 emit `ssh-terminal-closed`

### 2.4 SFTP（sftp.rs）

`ssh_sftp_open(conn_id)` → 复用连接会话、`channel_open_subsystem("sftp")`；其余
`list_dir / read_file / write_file / mkdir / rename / delete / stat`。读单次上限 8 MiB，
写按块（前端分块 + 进度条）。

### 2.5 AI（ai.rs）

复用 API Hub 已启用 Provider，无需重复配置凭据。

- 终端 AI：`ssh_ai_chat` 传入 `term_id` 附带最近屏幕输出作上下文，返回
  `{ reply, commands, dangerous, model, provider }`
- SFTP AI：`ssh_ai_sftp` 传入当前目录 JSON 列表，返回 `{ reply, actions }`（受支持动作）
- `ssh_ai_execute` — 在指定终端执行命令；`ssh_ai_list_models`；`ssh_ai_get_buffer`
- `is_dangerous` 危险命令检测（rm/mkfs/dd/sudo/…）；模型回复 `[DANGER]` 前缀亦触发确认

---

## 3. 前端结构

- `src/views/SSHConnections.vue` — 连接管理（分组 Tab、标签、测试、OpenSSH 导入/导出、双击/按钮开终端）
- `src/views/SSHTerminal.vue` — 多标签 xterm 终端 + AI 面板 + 端口转发(-L)/SOCKS(-D) 面板 + host key 确认
- `src/views/SSHSftp.vue` — 双栏文件浏览器 + 拖拽上传 + 分块下载 + SFTP AI 助手
- `src/lib/api-ssh.js` — 命令与事件封装（snake_case payload → camelCase）
- `src/lib/nav-config.js` — SSH 上下文子导航（连接/终端/文件）

事件：`ssh-terminal-output`、`ssh-terminal-closed`、`ssh-hostkey-prompt`。

---

## 4. 错误码

| 码 | 含义 |
|---|---|
| `AUTH_FAILED` | 认证失败 |
| `TIMEOUT` / `CONNECT_FAILED` | 连接超时/失败 |
| `HOST_KEY_MISMATCH` | 服务器指纹变更 |
| `HOSTKEY_REJECTED` | 首连未确认/被拒 |
| `KEY_INVALID` | 私钥解析失败 |
| `NO_SFTP` / `SFTP_*` | SFTP 会话/操作错误 |
| `NO_TERMINAL` / `NO_SESSION` | 句柄/会话不存在 |

---

## 5. 测试

- `cargo test ssh::` 覆盖：凭据加密 roundtrip、`SshConnectionInfo` 脱敏、host key 校验语义、known_hosts 落盘
- 前端 `pnpm check` 构建验证