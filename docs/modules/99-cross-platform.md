# 跨平台开发指南

## 1. 平台总览

DevNexus 目前正式支持的三个桌面平台:

| 平台 | 代号 | 架构 | 最低版本 |
|------|------|------|---------|
| macOS | `macos` | x86_64, arm64 (Apple Silicon) | macOS 12 Monterey |
| Linux | `linux` | x86_64, aarch64 | glibc 2.28+ |
| Windows | `windows` | x86_64 | Windows 10 1809+ |

---

## 2. 跨平台策略总纲

DevNexus 的跨平台策略分为三层:

```
┌─────────────────────────────────────┐
│ Layer 3: 前端 (Svelte + Tailwind)    │ ← 完全跨平台，零平台相关代码
├─────────────────────────────────────┤
│ Layer 2: 通用逻辑                       │
│   system.rs, environment.rs,         │
│   mirror.rs, scheduler.rs            │ ← 数据结构和非平台逻辑复用
├─────────────────────────────────────┤
│ Layer 1: 平台相关实现                   │
│   #[cfg(...)] 条件编译                │ ← 每平台独立实现，三选一
└─────────────────────────────────────┘
```

### 2.1 编译时多态 (`#[cfg]`)

项目中最常用的跨平台技术：

```rust
// 三选一条件编译
#[cfg(target_os = "macos")]    fn list_ports_impl() { /* lsof */ }
#[cfg(target_os = "linux")]    fn list_ports_impl() { /* /proc/net/tcp */ }
#[cfg(target_os = "windows")]  fn list_ports_impl() { /* netstat -ano */ }

// 二选一条件编译
#[cfg(unix)]  → macOS + Linux
#[cfg(windows)] → Windows 单独

// 单一平台专用
#[cfg(target_os = "macos")]
```

### 2.2 运行时策略

当编译时判断不可用时，使用运行时检测：

```rust
// 检测命令是否存在
if which::which("brew").is_ok() { /* Homebrew */ }
if which::which("python3").is_ok() { /* Python */ }

// 跨平台的文件路径处理
let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
// macOS:   ~/Library/Application Support/devnexus
// Linux:   ~/.local/share/devnexus
// Windows: C:\Users\xxx\AppData\Roaming\devnexus
```

### 2.3 条件编译 + 运行时双保险模式

```rust
#[cfg(target_os = "macos")]
fn elevate_impl() {
    Command::new("osascript")...
}

#[cfg(target_os = "linux")]
fn elevate_impl() {
    Command::new("pkexec")...
    .or_else(|| Command::new("sudo"))...
}

#[cfg(target_os = "windows")]
fn elevate_impl() {
    Command::new("powershell")
        .args(["-Command", "Start-Process -Verb RunAs"])...
}
```

**原则**: 编译时锁定 API 调用，运行时根据返回值做优雅降级。

---

## 3. 各平台文件系统差异

### 3.1 路径分隔符

| 方面 | Unix (macOS/Linux) | Windows |
|------|-------------------|---------|
| 分隔符 | `/` | `\` |
| Rust 处理 | 直接 `/` 字面量 | 使用 `std::path::PathBuf` / `Path::join()` |
| 环境变量 | `$HOME` | `%APPDATA%`, `%LOCALAPPDATA%` |

**项目中使用 `PathBuf::join` 统一处理**:

```rust
// ✅ 正确 — 自动选择正确的分隔符
let path = home.join("Library/Application Support/devnexus");  // macOS
// 等同于: /Users/lin/Library/Application Support/devnexus

// ✅ Windows 下自动转为 \
let path = appdata.join("devnexus/tasks.json");
// 等同于: C:\Users\lin\AppData\Roaming\devnexus\tasks.json
```

### 3.2 用户数据目录映射

```rust
pub fn data_dir() -> std::path::PathBuf {
    // 由 dirs crate 提供
    dirs::data_dir()  // macOS: ~/Library/Application Support
                      // Linux: ~/.local/share
                      // Windows: %APPDATA%
        .unwrap_or_else(|| std::env::current_dir().unwrap())
        .join("devnexus")
}
```

### 3.3 配置文件路径

| 文件/目录 | macOS | Linux | Windows |
|-----------|-------|-------|---------|
| npmrc | `~/.npmrc` | `~/.npmrc` | `~\.npmrc` |
| pip conf | `~/.pip/pip.conf` | `~/.config/pip/pip.conf` | `~\pip\pip.ini` |
| cargo config | `~/.cargo/config.toml` | `~/.cargo/config.toml` | `~\.cargo\config.toml` |
| Docker daemon | `/etc/docker/daemon.json` | `/etc/docker/daemon.json` | `%PROGRAMDATA%\Docker\config\daemon.json` |
| gemrc | `~/.gemrc` | `~/.gemrc` | `~\.gemrc` |
| condarc | `~/.condarc` | `~/.condarc` | `~\.condarc` |
| NuGet config | `~/.nuget/NuGet/NuGet.Config` | `~/.nuget/NuGet/NuGet.Config` | `%APPDATA%\NuGet\NuGet.Config` |

---

## 4. 跨平台命令执行模式

### 4.1 命令版本模式

```rust
/// 运行命令并返回 stdout（如果成功）
fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd).args(args).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}
```

**返回值 `Option<String>` 的意义**: 命令可能未安装、未在 PATH 中、执行失败或输出为空，返回 `None` 表示"不可用/未找到"，调用方据此做降级。

### 4.2 提权执行模式

| 平台 | 方案 | 说明 |
|------|------|------|
| macOS | 直接执行 | Homebrew 安装到 `/opt/homebrew`，无需 root |
| Linux | `pkexec` → `sudo -S` | pkexec 提供 GUI 弹窗；回退 sudo |
| Windows | `Start-Process -Verb RunAs` | 触发 UAC 弹窗 |

```rust
// 整体模式
fn run_elevated(binary: &str, args: &[&str]) -> Result<Output, String> {
    #[cfg(target_os = "macos")]
    { Command::new(binary).args(args).output().map_err(...) }

    #[cfg(target_os = "linux")]
    { Command::new("pkexec").arg(binary).args(args).output()
        .or_else(|_| { /* 回退 sudo -S */ }) }

    #[cfg(target_os = "windows")]
    { /* PowerShell Start-Process -Verb RunAs */ }
}
```

### 4.3 进程管理

| 操作 | Unix | Windows |
|------|------|---------|
| 杀进程 (优雅) | `SIGTERM` via `libc::kill` | `taskkill /PID` |
| 杀进程 (强制) | `SIGKILL` via `libc::kill` | `taskkill /PID /F` |
| 进程列表 | `sysinfo::System.processes()` | `sysinfo::System.processes()` |

Unix 两阶段杀进程（SIGTERM → 等待 2s → SIGKILL）:
- SIGTERM (15): 允许进程注册 signal handler 做资源清理
- SIGKILL (9): 内核直接杀死，进程无法拦截

Windows 没有等价的优雅/强制区分，`taskkill /F` 即为强制结束；`sysinfo` 的 `process.kill()` 调用 Win32 `TerminateProcess`。

---

## 5. 每个模块的跨平台分布

### 5.1 条件编译使用统计

| 模块 | `#[cfg(target_os = "macos")]` | `#[cfg(target_os = "linux")]` | `#[cfg(target_os = "windows")]` | `#[cfg(unix)]` | 纯共用 |
|------|:---:|:---:|:---:|:---:|:---:|
| system | 0 | 0 | 0 | 0 | 100% |
| software | 5 | 5 | 3 | 2 | 70% |
| environment | 1 | 1 | 1 | 2 | 90% |
| mirror | 2 | 1 | 1 | 0 | 85% |
| port | 0 | 0 | 0 | 2 | 80% |
| scheduler | 0 | 0 | 0 | 1 | 95% |
| password | 0 | 0 | 0 | 0 | 100% |
| cookie | 2 | 2 | 2 | 0 | 40% |
| residue | 1 | 1 | 1 | 1 | 30% |
| version | 1 | 1 | 1 | 0 | 70% |

**分析**:
- **最高 (100%)**: system, password — 纯算法/数据结构，不涉及外部命令
- **最低 (30%)**: residue (应用卸载残留) — 每个平台残留路径完全不同
- **中等 (70-90%)**: software, env, mirror, scheduler — 少量平台特定配置
- **复杂 (40%)**: cookie — 每个浏览器在每个平台的加密机制和存储路径都不同

---

## 6. 新增平台支持须知

### 6.1 需要修改的文件

| 修改类型 | 文件 | 修改内容 |
|---------|------|---------|
| 包管理器检测 | `software.rs` | `detect_package_managers` 中添加新平台包管理器 |
| 软件残留路径 | `software.rs` / `residue` | 每个软件添加新平台的残留路径 |
| 端口枚举 | `port_manager.rs` | 添加新平台的端口列表命令解析 |
| 系统操作 | `scheduler.rs` | 关机/休眠/重启的命令 |
| 浏览器 Cookie | `cookie_manager.rs` | 浏览器安装路径和加密机制 |
| Docker 配置 | `mirror.rs` | Docker daemon 配置文件路径 |
| 用户数据 | `utils.rs` | `data_dir()` 的实现 |

### 6.2 回归测试清单

- [ ] `cargo build` 在所有三个平台编译通过
- [ ] `cargo test` 通过（注意条件编译的测试覆盖率）
- [ ] 系统仪表盘信息正确显示
- [ ] 软件中心包管理器检测正常
- [ ] 端口管理能列出监听端口
- [ ] 定时任务能正常执行
- [ ] Cookie 提取器能读取至少一个浏览器

---

## 7. Windows 特有注意事项

### 7.1 编码问题

Windows 默认使用 UTF-16，而 Rust 字符串是 UTF-8：

```rust
// 处理 Windows 命令输出
let output = Command::new("cmd").args(["/C", "echo", "你好"]).output()?;
// 某些 Windows 命令输出是 GBK/GB18030 编码
let text = String::from_utf8_lossy(&output.stdout);  // UTF-8 解码
// 如果乱码，可能需要 encoding_rs crate 做 GBK 转 UTF-8
```

### 7.2 权限模型

- **UAC**: Windows 使用用户账户控制（UAC），需要提权的操作会弹窗确认
- **安装目录**: Program Files 需要管理员权限写入，建议安装到 `%LOCALAPPDATA%`
- **PATH 修改**: 通过注册表 `HKCU\Environment\PATH` 而非 shell 配置文件

### 7.3 路径处理

- `std::path::Path` 在 Windows 上正确处理 `C:\` 格式
- 注意 DLL 搜索路径: `SetDllDirectory` 或使用绝对路径加载动态库
- 避免在路径中使用 `\` 字面量（它是转义符），使用 `/` 或原始字符串 `r"C:\path"`

---

## 8. macOS 特有注意事项

### 8.1 系统完整性保护 (SIP)

- `/usr/bin/` 目录不可写（即使 sudo 也不行）
- 包管理器安装到 `/opt/homebrew`（ARM）或 `/usr/local`（Intel）
- Keychain 访问需要用户授权弹窗

### 8.2 Apple Silicon 兼容

- Rust 编译 target: `aarch64-apple-darwin`
- 通过 `universal2` 二进制支持 Intel/ARM 双架构
- 某些 sysinfo 在 Apple Silicon 上的 CPU 信息可能缺失

---

## 9. Linux 特有注意事项

### 9.1 发行版多样性

| 发行版 | 包管理器 | 桌面环境 | 注意 |
|--------|---------|---------|------|
| Ubuntu/Debian | apt | GNOME | 最广泛测试 |
| Fedora/RHEL | dnf | GNOME | 较通用 |
| Arch Linux | pacman | KDE | 滚动更新 |
| openSUSE | zypper | KDE | 较小众 |
| Alpine | apk | 无 | musl libc |

### 9.2 无桌面环境

如果 Tauri 在纯命令行下运行（无 X11/Wayland），某些功能（如 UAC 弹窗）需要特殊处理。

---

## 10. 跨平台依赖 crate

| crate | 用途 | 跨平台支持 |
|-------|------|-----------|
| `sysinfo` | 系统信息、进程 | ✅ 全平台 |
| `dirs` | 标准目录路径 | ✅ 全平台 |
| `which` | PATH 搜索 | ✅ 全平台 |
| `reqwest` | HTTP 请求 | ✅ 全平台 |
| `serde` / `serde_json` | 序列化 | ✅ 纯算法 |
| `rusqlite` | SQLite 操作 | ✅ 全平台 |
| `aes-gcm` | 加密 | ✅ 纯算法 |
| `pbkdf2` | 密钥派生 | ✅ 纯算法 |
| `chrono` | 时间处理 | ✅ 全平台 |
| `cron` | Cron 表达式 | ✅ 纯算法 |
| `uuid` | UUID 生成 | ✅ 全平台 |
| `tokio` | 异步运行时 | ✅ 全平台 |
| `tauri` | GUI 框架 | ✅ 全平台 |
