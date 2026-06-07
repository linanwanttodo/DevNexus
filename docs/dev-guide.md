# 开发者指南

## 1. 开发环境搭建

### 1.1 前置依赖

| 依赖 | 版本要求 | 用途 |
|------|---------|------|
| Rust | 1.77+ | 后端编译 |
| Node.js | 18+ | 前端构建 |
| pnpm | 9+ | 包管理器 |
| Tauri CLI | 2.x | 应用构建 |

### 1.2 安装

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装 pnpm
npm install -g pnpm

# 安装 Tauri CLI
cargo install tauri-cli --version "^2"

# 安装前端依赖
pnpm install
```

### 1.3 启动开发环境

```bash
# 同时启动前端 dev server 和 Tauri 窗口
cargo tauri dev

# 或者分别启动（先前端）
pnpm dev          # 启动 Vite 开发服务器 (localhost:1420)
cargo tauri dev   # 启动 Tauri 窗口，连接 Vite 服务器
```

---

## 2. 项目结构

```
devnexus/
├── src/                          # 前端 (Svelte 5)
│   ├── app.html                  # HTML 入口
│   ├── app.css                   # 全局样式 (Tailwind)
│   ├── lib/
│   │   ├── components/           # 可复用组件
│   │   │   ├── BrandIcons.svelte # 软件品牌图标映射
│   │   │   ├── Controls.svelte   # 通用控件
│   │   │   ├── Favorite.svelte   # 收藏按钮
│   │   │   ├── Loading.svelte    # 加载动画
│   │   │   ├── Notification.svelte # 通知提示
│   │   │   └── Sidebar.svelte    # 侧边导航栏
│   │   └── api.js                # Tauri IPC 调用封装
│   └── routes/                   # 页面组件（每个模块一个）
│       ├── +page.svelte          # 布局入口
│       ├── +layout.svelte        # 布局（侧边栏 + 内容区）
│       ├── Dashboard.svelte      # 系统仪表板
│       ├── SoftwareCenter.svelte # 软件中心
│       ├── EnvironmentManager.svelte # 环境管理
│       ├── MirrorSettings.svelte # 镜像设置
│       ├── PortManager.svelte    # 端口管理
│       ├── TaskScheduler.svelte  # 任务调度
│       ├── PasswordManager.svelte# 密码管理器
│       ├── CookieViewer.svelte   # Cookie 提取
│       └── Settings.svelte       # 设置页
├── src-tauri/                    # 后端 (Rust)
│   ├── src/
│   │   ├── main.rs               # 入口，注册 Tauri 命令
│   │   ├── lib.rs                # 库入口
│   │   ├── utils.rs              # 通用工具函数
│   │   ├── system.rs             # 系统仪表板
│   │   ├── environment.rs        # 环境管理器
│   │   ├── port_manager.rs       # 端口管理
│   │   ├── scheduler.rs          # 任务调度
│   │   ├── version_manager.rs    # 版本管理器
│   │   ├── password_manager.rs   # 密码管理器
│   │   ├── cookie_manager.rs     # Cookie 提取
│   │   └── residue_scanner.rs    # 残留扫描
│   ├── commands/
│   │   ├── mod.rs                # 命令模块导出
│   │   ├── system.rs             # 系统仪表板命令
│   │   ├── software.rs           # 软件中心命令
│   │   ├── environment.rs        # 环境管理命令
│   │   ├── mirror.rs             # 镜像设置命令
│   │   ├── scheduler.rs          # 任务调度命令
│   │   ├── password_manager.rs   # 密码管理命令
│   │   ├── cookie_manager.rs     # Cookie 提取命令
│   │   └── version_manager.rs    # 版本管理命令
│   ├── Cargo.toml                # Rust 依赖
│   └── tauri.conf.json           # Tauri 配置
├── package.json                  # 前端依赖
├── svelte.config.js              # Svelte 配置
├── tailwind.config.js            # Tailwind 配置
├── vite.config.ts                # Vite 配置
└── docs/                         # 文档
    ├── architecture.md           # 架构总览
    ├── modules/                  # 模块详细设计
    │   ├── 01-system.md
    │   ├── 02-software.md
    │   ├── 03-environment.md
    │   ├── 04-mirror.md
    │   ├── 05-port.md
    │   ├── 06-scheduler.md
    │   ├── 07-password.md
    │   ├── 08-cookie.md
    │   ├── 09-uninstall.md
    │   ├── 10-version.md
    │   └── 99-cross-platform.md
    ├── dev-guide.md              # 本文件
    └── README.md                 # 项目总览
```

---

## 3. 新增一个功能模块

### 3.1 步骤

1. **后端**: 在 `src-tauri/commands/` 下创建 `your_module.rs`
2. **注册命令**: 在 `commands/mod.rs` 中导出，在 `main.rs` 或 `lib.rs` 中 `.invoke_handler()`
3. **前端页面**: 在 `src/routes/` 下创建 `YourModule.svelte`
4. **添加导航**: 在 `+layout.svelte` 的导航列表中添加入口
5. **注册路由**: 在 `+page.svelte` 的路由表中添加

### 3.2 示例: Tauri 命令注册

```rust
// src-tauri/src/commands/your_module.rs
use tauri::command;

#[command]
pub fn my_command(input: String) -> Result<String, String> {
    Ok(format!("Hello, {}!", input))
}
```

```rust
// src-tauri/src/commands/mod.rs
pub mod your_module;
pub use your_module::*;
```

```rust
// src-tauri/src/lib.rs 或 main.rs
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::my_command,
            // ... 其他命令
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## 4. 编码规范

### 4.1 Rust 后端

- **命名**: `snake_case` 函数 + 变量，`PascalCase` 类型 + 枚举
- **错误处理**: 返回 `Result<T, String>`，前端只收到字符串错误
- **跨平台代码**: 使用 `#[cfg(target_os = "xxx")]` 而非运行时判断
- **命令函数**: 标记 `#[tauri::command]`，参数实现 `Serialize` 或 `Deserialize`
- **异步**: 长时间操作（HTTP 请求、进程执行）使用 `async`，纯计算用同步
- **测试**: 每个模块有 `#[cfg(test)] mod tests { ... }`

### 4.2 Svelte 前端

- **命名**: `camelCase` 变量 + 函数，`PascalCase` 组件
- **状态管理**: 使用 `$state()` rune（Svelte 5）
- **派生值**: 使用 `$derived()` 替代 `$:` 标记
- **IPC 调用**: 封装在 `lib/api.js` 中
- **样式**: 使用 Tailwind CSS，遵循 `nx-*` 主题前缀

### 4.3 异步 IPC 模式

所有 Rust 命令从前端调用时都是异步的：

```javascript
// 正确 — await
const result = await invoke("my_command", { input: "world" });

// 错误 — 不使用 await
const result = invoke("my_command", { input: "world" });  // 返回 Promise
```

---

## 5. 构建与发布

### 5.1 构建命令

```bash
# 开发构建（快速）
cargo tauri build --debug

# 发布构建（优化）
cargo tauri build --release

# 仅构建前端
pnpm build

# 仅检查后端编译
cargo check
```

### 5.2 构建产物

| 平台 | 产物 |
|------|------|
| macOS | `target/release/bundle/dmg/devnexus.dmg` |
| Linux | `target/release/bundle/deb/devnexus.deb`,
|       | `target/release/bundle/appimage/devnexus.AppImage` |
| Windows | `target/release/bundle/msi/devnexus.msi` |

---

## 6. 调试技巧

### 6.1 查看 Tauri 日志

```bash
# 启动时设置日志级别
RUST_LOG=debug cargo tauri dev

# 在运行时打开开发者工具
# 浏览器窗口右键 → Inspect Element
# 或在 Tauri 菜单中 View → Toggle Developer Tools
```

### 6.2 检查 IPC 通信

在前端添加调试日志：

```javascript
console.log("invoke: my_command", { input: "test" });
const result = await invoke("my_command", { input: "test" });
console.log("result:", result);
```

在 Rust 端添加日志（如果启用了 `log` crate + Tauri 的日志系统）：

```rust
#[command]
pub fn my_command(input: String) -> Result<String, String> {
    log::info!("my_command called with: {}", input);
    // ...
}
```

### 6.3 单独测试后端

```bash
cargo test             # 全部测试
cargo test -- --nocapture  # 显示 stdout
cargo test test_name   # 单个测试
```

---

## 7. 常见问题

### Q: macOS 上 `lsof` 不可用？

A: macOS 自带的 lsof 位于 `/usr/sbin/lsof`，但可能不在普通用户的 PATH 中。`software.rs` 中的端口检测使用 `which` crate 动态查找，如果找不到会显示空列表而非崩溃。

### Q: Windows 编译时 `openssl` 报错？

A: 安装 vcpkg 或使用 `openssl-sys` 的 vendored 特性：

```bash
# Cargo.toml
openssl = { version = "0.10", features = ["vendored"] }
```

### Q: 前端页面无法加载（空白页）？

A: 检查浏览器开发者工具 Console 面板。常见原因：
- Tauri IPC 连接尚未就绪（等待 `app.ready()`）
- Rust 命令 `#[tauri::command]` 忘记注册
- 前端路由表未包含该页面

### Q: `cargo check` 在 Windows 上失败？

A: 确认安装了 [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) 运行时（Windows 10 1809+ 已内置），以及 C++ 构建工具链（Visual Studio Build Tools）。

---

## 8. 依赖管理

### 8.1 更新前端依赖

```bash
pnpm update --latest
```

### 8.2 更新 Rust 依赖

```bash
cargo update
```

### 8.3 主要依赖清单

| 前端依赖 | 用途 |
|---------|------|
| `svelte` ^5 | 前端框架 |
| `@tauri-apps/api` ^2 | Tauri IPC |
| `tailwindcss` ^3 | CSS 框架 |
| `lucide-svelte` | 图标库 |

| Rust 依赖 | 用途 |
|----------|------|
| `tauri` ^2 | GUI 框架 |
| `sysinfo` | 系统信息 |
| `serde` + `serde_json` | 序列化 |
| `reqwest` | HTTP 请求 |
| `rusqlite` | SQLite |
| `aes-gcm` | AES 加密 |
| `pbkdf2` | 密钥派生 |
| `cron` | 定时任务 |
| `chrono` | 日期时间 |
| `tokio` | 异步运行时 |
