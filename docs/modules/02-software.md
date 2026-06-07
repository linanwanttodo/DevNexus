# 软件中心 — 模块设计文档

## 1. 功能概述

软件中心（Software Center）允许用户浏览、安装、卸载开发者常用工具。支持跨平台包管理器自动检测、智能包名映射、深度卸载残留清理、从 URL 安装便携式工具。

**通信链路**:
```
SoftwareCenter.svelte ──→ invoke("list_software")       ──→ software.rs
                     ──→ invoke("install_software")     ──→ software.rs
                     ──→ invoke("uninstall_software")   ──→ software.rs
                     ──→ invoke("uninstall_software_deep") ──→ software.rs + residue_scanner/
                     ──→ invoke("list_package_managers")──→ software.rs
```

---

## 2. 数据结构

```rust
pub struct Software {
    pub name: String,           // "Visual Studio Code"
    pub version: String,        // "1.93.0"
    pub category: String,       // "ide" / "database" / "cli" / "runtime"
    pub status: String,         // "installed" / "available" / "system"
    pub action: String,         // "Install" / "Uninstall" / "System Managed"
    pub package_name: String,   // "code" (传递给包管理器的底层包名)
}

struct SoftwareDef {
    name: &'static str,
    cmd: &'static str,          // 可执行文件名称，用于检测是否已安装
    category: &'static str,
    package_name: &'static str,
}

struct PackageManager {
    name: &'static str,            // "Homebrew"
    binary: &'static str,          // "brew"
    needs_sudo: bool,              // 是否需要提权
    install_args: &'static [&'static str],   // ["install"]
    uninstall_args: &'static [&'static str], // ["uninstall"]
}
```

**前端对应** (`routes/SoftwareCenter.svelte`):

```javascript
let selectedCategory = $state("all");
let filterInstalled = $state(false);
let filterUpdates = $state(false);
let software = $state([]);

let filteredSoftware = $derived(
    software
        .filter(s => selectedCategory === "all" || s.category === selectedCategory)
        .filter(s => !filterInstalled || s.status === "installed")
        .filter(s => !filterUpdates || s.status !== "available")
);
```

---

## 3. 核心实现

### 3.1 软件定义系统

37 款内置工具定义，分四类：

```rust
fn build_software_defs() -> Vec<SoftwareDef> {
    let mut defs = Vec::with_capacity(24);
    // IDE/编辑器: VS Code, Neovim, Vim, Sublime, Zed, Postman, IntelliJ
    // 数据库: DBeaver, SQLite, PostgreSQL, Redis, MySQL Workbench, TablePlus
    // CLI工具: Git, curl, wget, OpenSSH, GCC, Clang, CMake, htop, tmux,
    //          ripgrep, fd, jq, fzf
    // 运行时: Node.js, Python 3, Go, Rust, Java, Ruby, Docker
}
```

**平台条件编译**: Postman 和 IntelliJ IDEA 仅对 Windows/macOS 开放编译：

```rust
#[cfg(any(target_os = "windows", target_os = "macos"))]
{ defs.push(/* Postman */); defs.push(/* IntelliJ */); }
```

### 3.2 软件状态检测

```rust
fn list_software_impl() -> Vec<Software> {
    // 1. 获取已安装列表（从系统包管理器）
    let installed = get_installed_packages();
    // 2. 遍历预定义的 SoftwareDef
    for def in build_software_defs() {
        // 通过 which crate 检查 cmd 是否在 PATH 中
        let found = which::which(def.cmd).is_ok();
        // 确定 status 和 action
    }
    // 3. 遍历 installed 额外添加（非预定义工具也展示）
}
```

### 3.3 跨平台包管理器检测

```rust
fn detect_package_managers() -> Vec<PackageManager> {
    let mut managers = Vec::new();

    #[cfg(target_os = "macos")]
    { if which::which("brew").is_ok() { /* Homebrew */ }
      if which::which("port").is_ok() { /* MacPorts */ } }

    #[cfg(target_os = "linux")]
    { // 优先级: apt → dnf → pacman → zypper → apk
      if which::which("apt").is_ok() { /* apt */ }
      if which::which("dnf").is_ok() { /* dnf */ } ... }

    #[cfg(target_os = "windows")]
    { if which::which("winget").is_ok() { /* winget */ }
      if which::which("choco").is_ok() { /* Chocolatey */ } }

    // 跨平台包管理器（任何系统都可能安装）
    if which::which("pip3").is_ok() || which::which("pip").is_ok() { /* pip */ }
    if which::which("npm").is_ok() { /* npm */ }
    if which::which("cargo").is_ok() { /* cargo */ }
    if which::which("go").is_ok() { /* go install */ }
}
```

**检测优先级**：
- macOS: Homebrew > MacPorts
- Linux: apt > dnf > pacman > zypper > apk > snap > flatpak
- Windows: winget > Chocolatey

### 3.4 智能包名映射

```rust
fn map_package_name<'a>(generic: &'a str, pm_name: &str) -> &'a str {
    // 同一工具在不同包管理器下的包名不同
    match (generic, pm_name) {
        ("code", "apt") => "code",
        ("code", "Homebrew") => "visual-studio-code",
        ("code", "winget") => "Microsoft.VisualStudioCode",
        ("nodejs", "Homebrew") => "node",
        ("nodejs", "apt") => "nodejs",
        // ... 更多映射
    }
}
```

**设计原因**：同一个工具（如 VS Code），在 apt 下是 `code`，在 Homebrew 下是 `visual-studio-code`，在 winget 下是 `Microsoft.VisualStudioCode`。必须根据当前选择的包管理器做映射。

### 3.5 安装引擎

```rust
pub async fn install_software(package_name: String) -> Result<String, String> {
    let managers = detect_package_managers();
    // 依次尝试每个包管理器
    for pm in &managers {
        let pkg = map_package_name(&pkg_name, pm.name);
        let output = if pm.needs_sudo {
            run_elevated(pm.binary, &args)?   // 需要提权
        } else {
            Command::new(pm.binary).args(&args).output()?  // 直接执行
        };
        if output.status.success() {
            return Ok(format!("Successfully installed via {}", pm.name));
        }
    }
    Err("Failed with all package managers".to_string())
}
```

**设计决策**: 依次尝试所有可用包管理器，一个失败后自动尝试下一个。因为有些系统可能同时安装了 brew 和 pip，优先使用系统级包管理器。

### 3.6 跨平台提权执行

```rust
// macOS: brew 安装到 /opt/homebrew，不需要提权
#[cfg(target_os = "macos")]
fn run_elevated(binary: &str, args: &[&str]) -> Result<Output, String> {
    Command::new(binary).args(args).output().map_err(...)
}

// Linux: 需要 root 权限，使用 pkexec（图形化 GKT 弹窗）
#[cfg(target_os = "linux")]
fn run_elevated(binary: &str, args: &[&str]) -> Result<Output, String> {
    Command::new("pkexec").arg(binary).args(args).output()
        .or_else(|_| {
            // 回退到 sudo -S（需要终端输入密码）
            let mut child = Command::new("sudo")
                .arg("-S").arg(binary).args(args)
                .stdin(Stdio::inherit())
                .spawn()?;
            child.wait()?;
            // ...
        })
}

// Windows: UAC 提权
#[cfg(target_os = "windows")]
fn run_elevated(binary: &str, args: &[&str]) -> Result<Output, String> {
    // 使用 PowerShell Start-Process -Verb RunAs
    let ps_args = format!("Start-Process '{}' -ArgumentList '{}' -Verb RunAs -Wait -NoNewWindow",
        binary, args.join(" "));
    Command::new("powershell")
        .args(["-Command", &ps_args])
        .output()
        .map_err(...)
}
```

### 3.7 已安装应用枚举

```rust
pub async fn list_installed_apps() -> Result<Vec<InstalledApp>, String> {
    let managers = detect_package_managers();
    let mut apps = Vec::new();
    for pm in managers {
        let output = Command::new(pm.binary)
            .args(get_pm_list_args(pm.name)) // 不同包管理器用不同参数
            .output()?;
        let parsed = parse_pm_list_output(pm.name, &output_str);
        apps.extend(parsed);
    }
    Ok(apps)
}
```

**各包管理器列表命令格式解析**:

```rust
fn parse_pm_list_output(pm_name: &str, output: &str) -> Vec<(String, String)> {
    match pm_name {
        "apt" => {
            // "foo/stable,now 1.2.3 amd64 [installed]" → ("foo", "1.2.3")
        }
        "pacman" => {
            // "foo 1.2.3" → ("foo", "1.2.3")
            // "local/baz 3.0.0" → ("baz", "3.0.0")
        }
        "Homebrew" => {
            // "foo 1.2.3\nbar 1.0" → ("foo", "1.2.3")
        }
        // ...
    }
}
```

正则 vs 手动解析：因格式简单且固定，手动字符串分割比正则更高效。

### 3.8 从 URL 安装

```rust
pub async fn install_software_from_url(
    package_name: String,
    url: String,
    version: String,
    binary_name: String,
    archive_type: String,    // "tar.gz", "zip", "tar.xz"
    platform_hint: String,   // "darwin_amd64", "linux_arm64", "windows_amd64"
) -> Result<String, String> {
    // 1. 下载归档文件 (reqwest)
    // 2. 解压到临时目录 (tar/flate2/zip)
    // 3. 移动二进制到 ~/.local/bin (Unix) 或 LOCALAPPDATA (Windows)
    // 4. 设置可执行权限 (Unix)
    // 5. 添加到 PATH（如果不在）
}
```

**支持场景**: 对于没有包管理器的系统、或需要通过 GitHub Release 下载安装的便携式工具（如 `ripgrep`、`fd` 等）。

---

## 4. 深度卸载与残留清理

### 4.1 深度卸载流程

```rust
pub async fn force_uninstall_software(
    package_name: String,
    app_name: String,
) -> Result<String, String> {
    let mut messages = Vec::new();

    // 1. 杀残留进程
    let killed = kill_processes_by_name(&name_lower);
    if killed > 0 { messages.push(format!("Killed {} process(es)", killed)); }

    // 2. 包管理器卸载（可能失败，但继续清理残留）
    match uninstall_software(package_name.clone()).await {
        Ok(m) => messages.push(m),
        Err(e) => messages.push(format!("Package manager: {}", e)),
    }

    // 3. 扫描残留（跨平台，详见 residue_scanner）
    let scan = residue_scanner::scan_for_residues(&app_name, &package_name);

    // 4. 逐项删除
    for item in &scan.files  { std::fs::remove_file(&item.path); }
    for item in &scan.directories { std::fs::remove_dir_all(&item.path); }
    for item in &scan.shortcuts { if item.is_safe_to_delete { remove_file } }
    for item in &scan.services  { if item.is_safe_to_delete { remove_file } }
    #[cfg(windows)]
    for item in &scan.registry_keys { /* 删除注册表键 */ }

    Ok(format!("Cleanup complete:\n{}", messages.join("\n")))
}
```

### 4.2 残留进程查杀

```rust
fn kill_processes_by_name(name_lower: &str) -> usize {
    let keywords = name_lower.split(|c| c == ' ' || c == '-' || c == '_')
        .filter(|s| s.len() >= 3).collect();
    let mut killed = 0;
    for process in System::new().processes().values() {
        let pname = process.name().to_string_lossy().to_lowercase();
        // 关键词匹配 + 跳过自身进程
        if keywords.iter().any(|kw| pname.contains(kw)) {
            #[cfg(unix)] { process.kill_with(Signal::Term); process.kill_with(Signal::Kill); }
            #[cfg(windows)] { process.kill(); }
            killed += 1;
        }
    }
    killed
}
```

Unix 下先 SIGTERM 再 SIGKILL 双重保障。Windows 下 `process.kill()` 对应 `TerminateProcess`。

### 4.3 每个工具的精确残留路径

`get_cleanup_paths` 为每个已知工具定义了跨平台的残留路径。以 Node.js 为例：

```rust
"node.js" | "nodejs" => {
    // 跨平台通用
    paths.push(home.join(".npm"));
    paths.push(home.join(".node-gyp"));

    #[cfg(unix)]
    { paths.push("/usr/local/lib/node_modules"); }

    #[cfg(target_os = "macos")]
    { paths.push(home.join("Library/Caches/node")); }

    #[cfg(windows)]
    { paths.push(PathBuf::from(appdata).join("npm-cache")); }
}
```

---

## 5. 前端实现

### 5.1 分类筛选与搜索

```javascript
const categories = [
    { id: "all", label: "All" },
    { id: "ide", label: "IDE/Editor" },
    { id: "database", label: "Database" },
    { id: "cli", label: "CLI Tools" },
    { id: "runtime", label: "Runtimes" },
];
```

### 5.2 安装/卸载确认

卸载前会弹出两步确认链：

```javascript
// 第一步：确认卸载
await showConfirm(`Uninstall ${item.name}?`);
// 第二步：确认是否也删除配置和数据
const removeData = await showConfirm(`Also remove config and data files?`);
// 如果确认 → invoke("uninstall_software_deep")
// 如果否认 → invoke("uninstall_software")
```

### 5.3 品牌图标映射

通过 `BrandIcons.svelte` 为每个软件显示对应的品牌 SVG 图标：

```javascript
const map = {
    'Visual Studio Code': 'vscode',
    'Python 3': 'python',
    'Go': 'go',
    'Rust': 'rust',
    'Docker Desktop': 'docker',
    // ...
};
```

---

## 6. 测试

```rust
#[test] fn test_parse_apt_output()
#[test] fn test_parse_pacman_output()
#[test] fn test_parse_dnf_output()
#[test] fn test_parse_homebrew_output()
#[test] fn test_parse_winget_output()
#[test] fn test_parse_snap_output()
#[test] fn test_parse_flatpak_output()
#[test] fn test_parse_apk_output()
#[test] fn test_parse_zypper_output()
#[test] fn test_map_vscode()
#[test] fn test_map_nodejs()
#[test] fn test_map_python()
#[test] fn test_map_golang()
#[test] fn test_map_git()
```

**测试策略**: 每个包管理器的输出解析独立测试 + 每个包名映射独立测试。测试数据使用硬编码的模拟输出字符串，不依赖外部命令。

---

## 7. 跨平台总结

| 功能 | macOS | Linux | Windows |
|------|-------|-------|---------|
| 包管理器检测 | brew, port | apt, dnf, pacman, zypper, apk, snap, flatpak | winget, choco |
| 提权 | 不需要 | pkexec → sudo | UAC PowerShell |
| 残留路径格式 | `~/Library/...` | `~/.config/...` | `%APPDATA%/...` |
| 进程查杀 | SIGTERM → SIGKILL | SIGTERM → SIGKILL | TerminateProcess |
| 注册表清理 | ❌ | ❌ | ✅ |
| 服务文件清理 | launchd | systemd | ❌ |
