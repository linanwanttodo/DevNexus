# 环境管理 — 模块设计文档

## 1. 功能概述

环境管理器（Environment Manager）自动检测系统中安装的开发环境（语言运行时、工具链），显示版本和路径信息，支持添加/移除 PATH 环境变量。

**通信链路**:
```
EnvironmentManager.svelte ──→ invoke("list_environments")   ──→ environment.rs
                         ──→ invoke("add_to_path")          ──→ environment.rs
                         ──→ invoke("remove_from_path")     ──→ environment.rs
```

---

## 2. 数据结构

```rust
#[derive(Serialize)]
pub struct Environment {
    pub name: String,            // "Python 3"
    pub version: String,         // "3.12.0"
    pub path: String,            // "/usr/local/bin/python3"
    pub status: String,          // "Active" / "Not Found"
    pub shell_config: Option<String>,  // "~/.zshrc" (如果有对应 shell 配置)
    pub lang_type: String,       // "python" | "node" | "java" | "go" |
                                 // "rust" | "ruby" | "docker" | "git" | "cpp"
}
```

**前端对应** (`routes/EnvironmentManager.svelte`):

```javascript
let environments = $state([]);

let filteredEnvs = $derived(
    environments.filter(env => {
        match filterType.value {
            "all" => true,
            "installed" => env.status === "Active",
            "missing" => env.status !== "Active",
        }
    })
);
```

---

## 3. 核心实现

### 3.1 环境检测引擎

```rust
fn detect_environment(
    name: &str,             // "Python 3"
    lang_type: &str,        // "python"
    check_cmd: &str,        // "python3"
    version_args: &[&str],  // ["--version"]
    config_files: &[&str],  // ["~/.zshrc", "~/.bashrc"]
) -> Option<Environment> {
    // 1. 用 find_cmd_path 在 PATH 中搜索可执行文件
    if let Some(path) = utils::find_cmd_path(check_cmd) {
        // 2. 执行版本命令获取版本号
        let version = get_version(check_cmd, version_args);
        // 3. 在 shell 配置文件中搜索相关配置
        let shell_config = config_files.iter()
            .find(|&file| {
                let resolved = file.strip_prefix("~/")
                    .map(|rest| format!("{}/{}", home, rest))
                    .unwrap_or_else(|| file.to_string());
                std::path::Path::new(&resolved).exists()
            })
            .map(|s| s.to_string());
        // 4. 返回结果
        Some(Environment { name, version, path, status: "Active", shell_config, lang_type })
    } else {
        None  // 未找到该可执行文件
    }
}
```

### 3.2 统一列表命令

```rust
#[tauri::command]
pub fn list_environments() -> Vec<Environment> {
    let mut list = Vec::new();

    // 每个环境调用 detect_environment
    list.push(detect_environment("Python 3", "python", "python3",
        &["--version"], &["~/.zshrc", "~/.bashrc"]));

    list.push(detect_environment("Node.js", "node", "node",
        &["--version"], &["~/.zshrc", "~/.bashrc"]));

    list.push(detect_environment("Java (JDK)", "java", "java",
        &["-version"], &["~/.zshrc", "~/.bashrc"]));

    list.push(detect_environment("Go", "go", "go",
        &["version"], &[]));

    // ... 更多环境
    list.into_iter().flatten().collect()
}
```

`flatten()` 将 `Option<Environment>` 转为 `Environment`，未安装的环境自动跳过。

### 3.3 版本获取函数

```rust
fn get_version(cmd: &str, args: &[&str]) -> String {
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                version.lines().next().unwrap_or("unknown").to_string()
            } else {
                "unknown".to_string()
            }
        }
        Err(_) => "not found".to_string(),
    }
}
```

**只取第一行**：大多数 `--version` 输出的第一行包含版本号（如 `Python 3.12.0`），后续行可能是许可证信息或构建细节。

---

## 4. 跨平台 PATH 编辑器

这是环境管理器的核心功能，在不同平台实现方案完全不同。

### 4.1 Unix 路径 (macOS/Linux)

```rust
#[cfg(not(target_os = "windows"))]
fn add_to_path_impl(env_name: &str, path: &str) -> Result<String, String> {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let home = utils::user_home();

    // 1. 检测用户 shell 类型
    let (profile_files, export_line) = match shell.as_str() {
        s if s.contains("zsh") => (
            vec![format!("{}/.zshrc", home)],
            format!("\nexport PATH=\"{}/$PATH\"  # {} devnexus\n", path, env_name),
        ),
        s if s.contains("fish") => (
            vec![format!("{}/.config/fish/config.fish", home)],
            // fish 语法不同: fish_add_path 或 set -gx
            format!("\nset -gx PATH \"{}\" $PATH  # {} devnexus\n", path, env_name),
        ),
        _ => (  // bash 默认
            vec![
                format!("{}/.bashrc", home),
                format!("{}/.bash_profile", home),
                format!("{}/.profile", home),
            ],
            format!("\nexport PATH=\"{}/$PATH\"  # {} devnexus\n", path, env_name),
        ),
    };

    // 2. 如果已存在同名注解，先移除旧的
    remove_path_line(&profile_files, env_name);

    // 3. 选择第一个存在的 profile 文件，或创建默认的
    let profile = profile_files.iter()
        .find(|f| std::path::Path::new(f).exists())
        .unwrap_or(&profile_files[0]);

    // 4. 追加 export 语句
    let existing = std::fs::read_to_string(profile).unwrap_or_default();
    std::fs::write(profile, format!("{}{}", existing, export_line))?;

    Ok(format!("Added {} to PATH in {}", env_name, profile))
}
```

**关键设计**:
- 使用 Shell 检测确定写入哪个配置文件
- 在 export 行后添加 `# devnexus` 标记，便于后续识别和移除
- 添加前先清除同名旧条目，避免重复累积

### 4.2 Windows 路径

```rust
#[cfg(target_os = "windows")]
fn add_to_path_impl(env_name: &str, path: &str) -> Result<String, String> {
    // 1. 获取当前 PATH
    let output = Command::new("powershell")
        .args(["-Command", "[Environment]::GetEnvironmentVariable('PATH', 'User')"])
        .output()?;
    let current_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // 2. 检查是否已在 PATH 中
    if current_path.split(';').any(|p| p.trim() == path.trim()) {
        return Ok(format!("{} already in PATH", path));
    }

    // 3. 添加新路径
    let new_path = format!("{};{}", path.trim(), current_path);
    let ps_cmd = format!(
        "[Environment]::SetEnvironmentVariable('PATH', '{}', 'User')",
        new_path.replace('\'', "''")
    );
    Command::new("powershell")
        .args(["-Command", &ps_cmd])
        .output()?;

    Ok(format!("Added {} to user PATH", path))
}
```

**Windows 与 Unix 的核心差异**:

| 维度 | Unix | Windows |
|------|------|---------|
| 存储位置 | Shell 配置文件 (`.zshrc` 等) | 注册表 (用户级) |
| 格式 | `export PATH="/p:$PATH"` | `C:\p;%PATH%` |
| 分隔符 | `:` | `;` |
| 修改方式 | 文件 append | PowerShell .NET API |
| 即刻生效 | 需 `source` 或新终端 | 需重新打开进程 |

---

## 5. 路径移除逻辑

```rust
#[cfg(not(target_os = "windows"))]
fn remove_from_path_impl(env_name: &str, _path: &str) -> Result<String, String> {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let home = utils::user_home();
    let profiles = detect_profile_files(&shell, &home);

    let count = 0;
    for profile in &profiles {
        if !std::path::Path::new(profile).exists() { continue; }
        // 读取文件, 移除包含 `# env_name devnexus` 的行
        let content = std::fs::read_to_string(profile)?;
        let new_content = content.lines()
            .filter(|line| !line.contains(&format!("# {} devnexus", env_name)))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(profile, new_content)?;
        count += 1;
    }
    // ...
}
```

**设计要点**: 通过 `# devnexus` 标记精确识别此前由 DevNexus 添加的行，避免误删用户自己添加的 PATH。

---

## 6. 前端实现

### 6.1 环境展示卡片

每个环境展示：
```
┌────────────────────────────┐
│ [icon] Python 3    [Active]│
│        3.12.0              │
│        /usr/local/bin      │
│        Config: ~/.zshrc    │
│        [Switch] [Remove]   │
└────────────────────────────┘
```

### 6.2 版本切换

通过 `version_manager` 模块支持版本切换（如果该语言有版本管理器）：

```javascript
async function switchVersion(env, version) {
    await invoke("switch_version", { langType: env.lang_type, version: version.version });
    await loadVersions(env, true);  // 刷新缓存
}
```

### 6.3 添加自定义 PATH 的对话框

```javascript
<dialog>
  <h3>Add to PATH</h3>
  <input bind:value={newEnvName} placeholder="Environment name" />
  <input bind:value={newEnvPath} placeholder="/path/to/bin" />
  <button onclick={handleAddToPath}>Add</button>
</dialog>
```

---

## 7. 跨平台总结

| 环境 | macOS 检测 | Linux 检测 | Windows 检测 |
|------|-----------|-----------|-------------|
| Python | `python3 --version` | `python3 --version` | `python --version` |
| Node.js | `node --version` | `node --version` | `node --version` |
| Java | `java -version` | `java -version` | `java -version` |
| Go | `go version` | `go version` | `go version` |
| Rust | `rustc --version` | `rustc --version` | `rustc --version` |
| Docker | `docker --version` | `docker --version` | `docker --version` |
| Git | `git --version` | `git --version` | `git --version` |
| GCC | `gcc --version` | `gcc --version` | `gcc --version` |
| .NET | `dotnet --version` | `dotnet --version` | `dotnet --version` |
| Flutter | `flutter --version` | `flutter --version` | `flutter --version` |
| PHP | `php --version` | `php --version` | `php --version` |

> **注意**: 检测命令是跨平台相同的，区别仅在于可执行文件在系统中的路径位置。`utils::find_cmd_path` 已封装 PATH 查找的跨平台差异。
