# 版本管理 — 模块设计文档

## 1. 功能概述

版本管理器（Version Manager）为开发者常用的语言运行时（Python、Node.js、Java、Go、Rust、C/C++）提供多版本检测与切换能力。自动检测已安装的版本管理工具（如 `pyenv`、`fnm`、`nvm`、`jenv`、`gvm`、`rustup`），同时也支持通过 PATH 路径手动指定版本。

**通信链路**:
```
EnvironmentManager.svelte ──→ invoke("list_versions")    ──→ version_manager.rs
                         ──→ invoke("switch_version")    ──→ version_manager.rs
```

---

## 2. 数据结构

```rust
/// 单个版本信息
#[derive(Serialize, Debug, Clone)]
pub struct VersionInfo {
    pub version: String,    // "3.12.0"
    pub path: String,       // "/Users/me/.pyenv/versions/3.12.0/bin"
    pub is_active: bool,    // 当前是否激活
}

/// 版本语言
pub enum LangType {
    Python,
    Node,
    Java,
    Go,
    Rust,
    Cpp,
}
```

**前端对应** (`routes/EnvironmentManager.svelte`):

```javascript
let versionCache = $state({});   // { langType: { versions: [...], current: "..." } }

async function loadVersions(env, forceRefresh = false) {
    // 使用缓存 — 10 分钟 TTL
    if (!forceRefresh && versionCache[env.lang_type]?.timestamp > Date.now() - 600000) {
        return;
    }
    const versions = await invoke("list_versions", { langType: env.lang_type });
    versionCache = {
        ...versionCache,
        [env.lang_type]: { versions, timestamp: Date.now() },
    };
}
```

---

## 3. 核心实现（每种语言独立的检测与切换）

### 3.1 Python — pyenv

**设计目标**: 优先使用 `pyenv`（最主流的 Python 版本管理器），如果未安装则返回空列表。

```rust
fn list_python_versions() -> Vec<VersionInfo> {
    // 优先 pyenv
    if let Some(output) = run_cmd("pyenv", &["versions", "--bare"]) {
        let current = run_cmd("pyenv", &["version-name"]).unwrap_or_default();
        return output.lines().map(|v| v.trim()).filter(|v| !v.is_empty()).map(|v| {
            VersionInfo {
                version: v.to_string(),
                path: run_cmd("pyenv", &["prefix", v]).unwrap_or_default().trim().to_string(),
                is_active: v == current.trim(),
            }
        }).collect();
    }
    vec![]
}

fn switch_python_version(version: &str) -> Result<String, String> {
    // pyenv global <version>
    let output = Command::new("pyenv").args(["global", version]).output()
        .map_err(|e| format!("Failed to run pyenv: {}", e))?;
    if output.status.success() {
        Ok(format!("Python switched to {}", version))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
```

**`run_cmd` 辅助函数**:

```rust
fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd).args(args).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}
```

将命令执行包装为 `Option<String>`，失败时返回 `None`（而非 panic 或错误处理），后续通过 `?` 操作符短路。

### 3.2 Node.js — fnm / nvm

**设计目标**: 优先 `fnm`（Rust 实现，更快），回退 `nvm`。

```rust
fn list_node_versions() -> Vec<VersionInfo> {
    // 优先 fnm
    if let Some(output) = run_cmd("fnm", &["list", "--core18-enabled=false"]) {
        let current = run_cmd("fnm", &["current"]).unwrap_or_default();
        return output.lines().filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() { return None; }
            let (is_active, version) = if trimmed.starts_with(">") {
                (true, trimmed.trim_start_matches("> ").trim())
            } else if trimmed.starts_with("*") {
                (true, trimmed.trim_start_matches("* ").trim())
            } else {
                (false, trimmed)
            };
            let path = run_cmd("fnm", &["which", version]).unwrap_or_default();
            let path = path.trim().trim_end_matches("/bin/node").to_string();
            Some(VersionInfo { version: version.to_string(), path, is_active, ..Default::default() })
        }).collect();
    }

    // 回退 nvm（解析 ~/.nvm/alias 目录和 ~/.nvm/versions/node/*）
    // nvm 是 shell 函数，需要 bash -c 调用
    if let Some(versions) = list_nvm_versions() { return versions; }
    vec![]
}

fn switch_node_version(version: &str) -> Result<String, String> {
    if which::which("fnm").is_ok() {
        let output = Command::new("fnm")
            .args(["use", version, "--core18-enabled=false"]).output()?;
    } else {
        // nvm use <version> (需要 source nvm.sh)
        let output = Command::new("bash")
            .args(["-c", &format!("source ~/.nvm/nvm.sh && nvm use {}", version)])
            .output()?;
    }
    Ok(())
}
```

**注意**: nvm 是一个 shell 函数而非独立的可执行文件，因此需要 `bash -c "source nvm.sh && nvm use"` 方式调用。

### 3.3 Java — jenv / 手动检测

```rust
fn list_java_versions() -> Vec<VersionInfo> {
    // 优先 jenv
    if let Some(output) = run_cmd("jenv", &["versions", "--bare"]) {
        let current = run_cmd("jenv", &["version-name"]).unwrap_or_default();
        return output.lines().filter_map(|line| {
            let v = line.trim();
            if v.is_empty() { return None; }
            Some(VersionInfo {
                version: v.to_string(),
                is_active: v == current.trim(),
                path: find_java_home_for_version(v).unwrap_or_default().to_string_lossy().to_string(),
            })
        }).collect();
    }

    // 回退: 在标准 JDK 安装目录中搜索
    let search_dirs = build_jvm_search_dirs();
    let mut versions = Vec::new();
    for dir in search_dirs {
        if !dir.exists() { continue; }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let java_bin = entry.path().join("bin/java");
                if java_bin.exists() {
                    let version = get_java_version(&entry.path());
                    versions.push(VersionInfo {
                        version, path: entry.path().to_string_lossy().to_string(),
                        is_active: false,
                    });
                }
            }
        }
    }
    versions
}
```

**标准 JDK 搜索目录**:

```rust
fn build_jvm_search_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(target_os = "macos")]
    dirs.push(std::path::PathBuf::from("/Library/Java/JavaVirtualMachines"));
    #[cfg(target_os = "linux")]
    dirs.extend([
        std::path::PathBuf::from("/usr/lib/jvm"),
        std::path::PathBuf::from("/usr/java"),
    ]);
    #[cfg(target_os = "windows")]
    dirs.extend([
        std::path::PathBuf::from("C:\\Program Files\\Java"),
        std::path::PathBuf::from("C:\\Program Files (x86)\\Java"),
    ]);
    dirs.push(std::path::PathBuf::from(user_home()).join(".sdkman/candidates/java"));
    dirs
}
```

**Java 版本切换 — 跨平台处理**:

```rust
fn switch_java_version(version: &str) -> Result<String, String> {
    // jenv (macOS/Linux)
    if which::which("jenv").is_ok() {
        return run_and_check("jenv", &["global", version]);
    }

    // macOS: 使用 /usr/libexec/java_home -v <version>
    #[cfg(target_os = "macos")] {
        let java_home = Command::new("/usr/libexec/java_home")
            .args(["-v", version]).output()?;
        let java_home = String::from_utf8_lossy(&java_home.stdout).trim().to_string();
        upsert_shell_config("JAVA_HOME", &format!("export JAVA_HOME=\"{}\"", java_home))?;
    }

    // Windows: setx JAVA_HOME <path>
    #[cfg(target_os = "windows")] {
        if let Some(path) = find_java_home_for_version(version) {
            Command::new("setx").args(["JAVA_HOME", &path]).output()?;
        }
    }

    Ok(format!("Java switched to {}", version))
}
```

### 3.4 Go — gvm

```rust
fn list_go_versions() -> Vec<VersionInfo> {
    // gvm list — 输出中 => 标记当前版本
    if let Some(output) = run_cmd("gvm", &["list"]) {
        return output.lines().filter_map(|line| {
            let trimmed = line.trim();
            if trimmed == "gvm" || trimmed.is_empty() { return None; }
            Some(VersionInfo {
                version: trimmed.trim_start_matches("=> ").to_string(),
                is_active: trimmed.starts_with("=>"),
                ..Default::default()
            })
        }).collect();
    }
    vec![]
}

fn switch_go_version(version: &str) -> Result<String, String> {
    run_and_check("gvm", &["use", version, "--default"])
}
```

### 3.5 Rust — rustup

```rust
fn list_rust_versions() -> Vec<VersionInfo> {
    // rustup toolchain list — 输出 "stable-x86_64-apple-darwin (default)"
    if let Some(output) = run_cmd("rustup", &["toolchain", "list"]) {
        return output.lines().map(|line| {
            let trimmed = line.trim();
            let is_default = trimmed.contains("(default)");
            let version = trimmed.split_whitespace().next().unwrap_or(trimmed);
            let channel = version.split('-').next().unwrap_or(version);
            VersionInfo {
                version: channel.to_string(),
                is_active: is_default || trimmed.contains("(default)"),
                ..Default::default()
            }
        }).collect();
    }
    vec![]
}

fn switch_rust_version(version: &str) -> Result<String, String> {
    // rustup default <toolchain>
    run_and_check("rustup", &["default", version])
}
```

### 3.6 C/C++ — 多编译器支持

```rust
fn list_cpp_versions() -> Vec<VersionInfo> {
    let mut versions = Vec::new();
    let home = user_home();

    // GCC
    if let Ok(output) = Command::new("gcc").arg("--version").output() {
        // 解析第一行获取版本号，如 "gcc (Ubuntu 11.4.0-1ubuntu1~22.04) 11.4.0"
        let version = parse_gcc_version(&String::from_utf8_lossy(&output.stdout));
        versions.push(VersionInfo {
            version: version.clone(),
            path: which::which("gcc").map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            is_active: true,
        });

        // 检测更多版本: /usr/bin/gcc-*
        if let Ok(entries) = glob("/usr/bin/gcc-[0-9]*") {
            for entry in entries {
                versions.push(VersionInfo { version: entry.trim_start_matches("/usr/bin/gcc-"), ... });
            }
        }
    }

    // Clang — 类似逻辑
    if let Ok(output) = Command::new("clang").arg("--version").output() {
        versions.push(VersionInfo { ... });
    }

    versions
}

fn switch_cpp_version(version: &str) -> Result<String, String> {
    // update-alternatives (Debian/Ubuntu) 或
    // 设置 CC/CXX 环境变量
    upsert_shell_config("cc/cxx devnexus",
        &format!("export CC=/usr/bin/gcc-{}\nexport CXX=/usr/bin/g++-{}", version, version))
}
```

---

## 4. Shell 配置文件操作

当版本切换需要持久化环境变量时，操作 Shell 配置文件:

```rust
fn upsert_shell_config(marker: &str, content: &str) -> Result<(), String> {
    let config_file = detect_shell_config();
    let config_path = format!("{}/{}", user_home(), config_file);
    let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
    let new_content = replace_section(&existing, marker, content);
    std::fs::write(&config_path, new_content)
        .map_err(|e| format!("Failed to write {}: {}", config_path, e))
}

fn replace_section(existing: &str, marker: &str, new_content: &str) -> String {
    let start_marker = format!("# BEGIN {} devnexus", marker);
    let end_marker = format!("# END {} devnexus", marker);

    if existing.contains(&start_marker) && existing.contains(&end_marker) {
        // 替换已存在的区域
        let before = existing.split(&start_marker).next().unwrap_or("");
        let after = existing.split(&end_marker).nth(1).unwrap_or("");
        format!("{}{}\n{}\n{}\n{}", before, start_marker, new_content, end_marker, after)
    } else {
        // 追加新区域
        format!("{}\n\n{}\n{}\n{}\n", existing.trim_end(), start_marker, new_content, end_marker)
    }
}
```

**设计要点**: 使用 `# BEGIN/END <marker> devnexus` 注释来包裹由 DevNexus 添加的环境变量配置块，这样：
1. 多次切换同一个语言时，只替换块内内容，不会重复积累
2. 用户手动删除块即可撤销 DevNexus 的所有改动
3. 不影响块外用户自己写的内容

---

## 5. 统一入口

```rust
#[tauri::command]
pub fn list_versions(lang_type: String) -> Vec<VersionInfo> {
    match lang_type.as_str() {
        "python" => list_python_versions(),
        "node"   => list_node_versions(),
        "java"   => list_java_versions(),
        "go"     => list_go_versions(),
        "rust"   => list_rust_versions(),
        "cpp"    => list_cpp_versions(),
        _        => vec![],
    }
}

#[tauri::command]
pub fn switch_version(lang_type: String, version: String) -> Result<String, String> {
    match lang_type.as_str() {
        "python" => switch_python_version(&version),
        "node"   => switch_node_version(&version),
        "java"   => switch_java_version(&version),
        "go"     => switch_go_version(&version),
        "rust"   => switch_rust_version(&version),
        "cpp"    => switch_cpp_version(&version),
        _        => Err("Unsupported language type".to_string()),
    }
}
```

---

## 6. 前端实现

### 6.1 版本列表弹窗

在 EnvironmentManager 中，点击环境的版本号区域展开版本列表:

```html
<dialog>
  <h3>Select {env.name} Version</h3>
  <div class="divide-y divide-nx-border">
    {#each versions as v}
      <button
        class="flex w-full items-center justify-between px-4 py-3"
        onclick={() => selectVersion(env, v)}
      >
        <div>
          <span>{v.version}</span>
          <span class="text-xs text-nx-text-muted">{v.path}</span>
        </div>
        <div>
          {#if v.is_active}
            <span class="text-nx-success">Active</span>
          {:else}
            <span>Switch</span>
          {/if}
        </div>
      </button>
    {/each}
  </div>
</dialog>
```

### 6.2 版本缓存

```javascript
let versionCache = $state({});

async function loadVersions(env, forceRefresh = false) {
    if (!forceRefresh && versionCache[env.lang_type]?.timestamp > Date.now() - 600000) {
        return; // 缓存 10 分钟
    }
    const versions = await invoke("list_versions", { langType: env.lang_type });
    versionCache = {
        ...versionCache,
        [env.lang_type]: { versions, timestamp: Date.now() },
    };
}
```

**为什么要缓存 10 分钟**:
- `list_versions` 每次调用都会执行外部命令（如 `pyenv versions`），开销远高于内存读取
- 用户在版本选择对话框里通常会在数秒内操作完毕，频繁刷新没有意义
- 切换版本后传递 `forceRefresh=true` 强制重新加载

---

## 7. 测试

```rust
#[test] fn test_list_python_versions_with_pyenv()
#[test] fn test_list_node_versions_with_fnm()
#[test] fn test_switch_python_version()
#[test] fn test_replace_section_new()
#[test] fn test_replace_section_existing()
```

测试覆盖: 各个语言列表和切换的基本路径，shell 配置的插入和替换逻辑。

---

## 8. 关键设计决策

1. **每种语言独立函数**: 6 种语言各自的检测逻辑完全不同（命令名、参数、输出格式），统一抽象的成本高于收益

2. **优先版本管理器方案**: pyenv / fnm / jenv / gvm / rustup 优于手动管理，因为版本管理器：
   - 提供了统一的多版本切换接口
   - 能隔离不同版本的依赖/编译产物
   - 无需用户手动配置 PATH

3. **`run_cmd` 返回 `Option`**: 外部命令可能失败（工具未安装、系统 PATH 未配置），使用 `Option` 优雅降级而非 panic

4. **`upsert_shell_config` 的幂等设计**: 多次 switch 同一个版本不会在配置文件中产生重复行
