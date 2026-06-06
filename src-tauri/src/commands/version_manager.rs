use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;

#[cfg(unix)]
use std::os::unix::fs::symlink;

/// 每个语言类型的版本数据缓存条目
#[derive(Serialize, Deserialize, Clone)]
struct CachedEntry {
    versions: Vec<VersionInfo>,
}

/// 版本缓存管理器（两级：内存 + 文件）
/// 缓存永久有效，直到用户手动刷新或切换版本
pub struct VersionCache {
    inner: Mutex<HashMap<String, CachedEntry>>,
    cache_path: Option<std::path::PathBuf>,
}

impl VersionCache {
    /// 创建缓存管理器，自动从文件加载持久化数据
    pub fn new() -> Self {
        let cache_path = dirs::cache_dir().map(|d| {
            let p = d.join("devnexus").join("version_cache.json");
            // 确保目录存在
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            p
        });

        // 从文件加载已有缓存
        let inner = cache_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<HashMap<String, CachedEntry>>(&s).ok())
            .unwrap_or_default();

        Self {
            inner: Mutex::new(inner),
            cache_path,
        }
    }

    /// 获取缓存（只要存在就返回，永不过期）
    fn get(&self, lang_type: &str) -> Option<Vec<VersionInfo>> {
        let inner = self.inner.lock().ok()?;
        inner.get(lang_type).map(|e| e.versions.clone())
    }

    /// 写入缓存并持久化到文件
    fn set(&self, lang_type: String, versions: Vec<VersionInfo>) {
        let entry = CachedEntry { versions };
        if let Ok(mut inner) = self.inner.lock() {
            inner.insert(lang_type, entry);
            // 持久化到文件
            if let Some(ref path) = self.cache_path.clone() {
                let data = serde_json::to_string(&*inner).unwrap_or_default();
                let _ = std::fs::write(path, data);
            }
        }
    }

    /// 失效特定语言类型的缓存（切换版本后调用）
    pub fn invalidate(&self, lang_type: &str) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.remove(lang_type);
            // 同步更新文件
            if let Some(ref path) = self.cache_path.clone() {
                let data = serde_json::to_string(&*inner).unwrap_or_default();
                let _ = std::fs::write(path, data);
            }
        }
    }
}

impl Default for VersionCache {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VersionInfo {
    pub version: String,
    pub path: String,
    pub is_active: bool,
}

/// 执行命令并返回标准输出
fn run_cmd(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

/// 列出指定语言的所有已安装版本（永久缓存，除非手动强制刷新）
#[tauri::command]
pub fn list_versions(
    lang_type: String,
    force_refresh: Option<bool>,
    cache: tauri::State<'_, VersionCache>,
) -> Vec<VersionInfo> {
    // 非强制刷新时优先返回缓存
    if force_refresh != Some(true) {
        if let Some(cached) = cache.get(&lang_type) {
            return cached;
        }
    }
    // 扫描真实环境
    let versions = match lang_type.as_str() {
        "python" => list_python_versions(),
        "node" => list_node_versions(),
        "java" => list_java_versions(),
        "go" => list_go_versions(),
        "rust" => list_rust_versions(),
        "cpp" => list_cpp_versions(),
        _ => vec![],
    };
    // 写入缓存并持久化到本地文件
    cache.set(lang_type, versions.clone());
    versions
}

/// 切换指定语言的活跃版本，切换后失效对应缓存
#[tauri::command]
pub fn switch_version(
    lang_type: String,
    version: String,
    cache: tauri::State<'_, VersionCache>,
) -> Result<String, String> {
    let result = match lang_type.as_str() {
        "python" => switch_python_version(&version),
        "node" => switch_node_version(&version),
        "java" => switch_java_version(&version),
        "go" => switch_go_version(&version),
        "rust" => switch_rust_version(&version),
        "cpp" => switch_cpp_version(&version),
        _ => Err(format!("Unsupported language: {}", lang_type)),
    };
    // 切换成功后失效缓存，下次打开时会重新扫描
    if result.is_ok() {
        cache.invalidate(&lang_type);
    }
    result
}

// ==================== Python (pyenv) ====================

fn list_python_versions() -> Vec<VersionInfo> {
    // 优先使用 pyenv
    if let Some(output) = run_cmd("pyenv", &["versions", "--bare"]) {
        let current = run_cmd("pyenv", &["version-name"])
            .unwrap_or_default()
            .trim()
            .to_string();
        return output
            .lines()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(|v| {
                let path = run_cmd("pyenv", &["prefix", v])
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                VersionInfo {
                    version: v.to_string(),
                    path,
                    is_active: v == current,
                }
            })
            .collect();
    }
    vec![]
}

fn switch_python_version(version: &str) -> Result<String, String> {
    let output = Command::new("pyenv")
        .args(["global", version])
        .output()
        .map_err(|e| format!("Failed to run pyenv: {}", e))?;
    if output.status.success() {
        Ok(format!("Python switched to {}", version))
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("pyenv switch failed: {}", err))
    }
}

// ==================== Node.js (fnm / nvm) ====================

fn list_node_versions() -> Vec<VersionInfo> {
    // 优先 fnm（Rust 编写，更快）
    if let Some(output) = run_cmd("fnm", &["list", "--core18-enabled=false"]) {
        let current = run_cmd("fnm", &["current"]).unwrap_or_default();
        let current = current.trim();
        return output
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("*") {
                    return None;
                }
                let (is_active, version) = if trimmed.starts_with(">") {
                    (true, trimmed.trim_start_matches("> ").trim())
                } else if trimmed.starts_with("*") {
                    (true, trimmed.trim_start_matches("* ").trim())
                } else {
                    (false, trimmed)
                };
                let version = version.trim();
                if version.is_empty() {
                    return None;
                }
                let path = run_cmd("fnm", &["which", version]).unwrap_or_default();
                Some(VersionInfo {
                    version: version.to_string(),
                    path: path.trim().to_string(),
                    is_active: is_active || version == current,
                })
            })
            .collect();
    }
    // 回退到 nvm
    if nvm_list_versions() != vec![] {
        return nvm_list_versions();
    }
    vec![]
}

fn nvm_list_versions() -> Vec<VersionInfo> {
    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "bash"
    };
    if let Some(output) = run_cmd(
        shell,
        &[
            "-c",
            "source $NVM_DIR/nvm.sh 2>/dev/null && nvm ls --no-colors 2>/dev/null",
        ],
    ) {
        let current = run_cmd("node", &["--version"])
            .unwrap_or_default()
            .trim()
            .to_string();
        return output
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.contains("->") || trimmed.contains("default") {
                    // nvm uses "-> v18.17.0" for current
                    let parts: Vec<&str> = trimmed.split("->").collect();
                    if parts.len() >= 2 {
                        let v = parts[1].trim().trim_end_matches(" (currently in use)");
                        return Some(VersionInfo {
                            version: v.to_string(),
                            path: String::new(),
                            is_active: true,
                        });
                    }
                    return None;
                }
                if trimmed.starts_with("v") || trimmed.starts_with(" ") {
                    let v = trimmed.trim();
                    return Some(VersionInfo {
                        version: v.to_string(),
                        path: String::new(),
                        is_active: v == current,
                    });
                }
                None
            })
            .collect();
    }
    vec![]
}

fn switch_node_version(version: &str) -> Result<String, String> {
    // 先试 fnm
    let _ = Command::new("fnm").args(["use", version]).output();
    // 再试 nvm
    if cfg!(unix) {
        let output = Command::new("bash")
            .args([
                "-c",
                &format!(
                    "source $NVM_DIR/nvm.sh 2>/dev/null && nvm use {} 1>/dev/null 2>&1",
                    version
                ),
            ])
            .output()
            .map_err(|e| format!("Failed to switch Node: {}", e))?;
        if output.status.success() {
            return Ok(format!("Node.js switched to {}", version));
        }
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("Node switch failed: {}", err))
    } else {
        Err("Node.js version switching not supported on this platform".to_string())
    }
}

// ==================== Java (jenv / SDKMAN / Homebrew / system) ====================

fn list_java_versions() -> Vec<VersionInfo> {
    // 1) 优先 jenv
    if let Some(output) = run_cmd("jenv", &["versions", "--bare"]) {
        let current = run_cmd("jenv", &["version-name"])
            .unwrap_or_default()
            .trim()
            .to_string();
        return output
            .lines()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .filter(|v| *v != "system")
            .map(|v| {
                let path = run_cmd("jenv", &["which", v]).unwrap_or_default();
                VersionInfo {
                    version: v.to_string(),
                    path: path.trim().to_string(),
                    is_active: v == current,
                }
            })
            .collect();
    }
    // 2) SDKMAN
    let sdkman = dirs::home_dir().map(|h| h.join(".sdkman").join("candidates").join("java"));
    let mut results = Vec::new();
    if let Some(ref sdkman_dir) = sdkman {
        if sdkman_dir.exists() {
            results.extend(scan_jdk_dir(sdkman_dir, true));
            if !results.is_empty() {
                return results;
            }
        }
    }
    // 3) 扫描所有常见 JVM 路径
    scan_all_jvm_dirs()
}

/// 扫描所有已知的 JVM 安装目录（跨平台，无硬编码单一路径）
fn scan_all_jvm_dirs() -> Vec<VersionInfo> {
    let current_java = resolve_current_java_path();
    let mut candidates = vec![
        // --- Linux ---
        std::path::PathBuf::from("/usr/lib/jvm"),
        std::path::PathBuf::from("/usr/java"),
        std::path::PathBuf::from("/opt/java"),
        std::path::PathBuf::from("/opt/jdk"),
        // --- macOS ---
        std::path::PathBuf::from("/Library/Java/JavaVirtualMachines"),
        // Homebrew
        std::path::PathBuf::from("/opt/homebrew/opt"),
        std::path::PathBuf::from("/usr/local/opt"),
        // --- Windows ---
        std::path::PathBuf::from(r"C:\Program Files\Java"),
        std::path::PathBuf::from(r"C:\Program Files\Eclipse Adoptium"),
        std::path::PathBuf::from(r"C:\Program Files\Amazon Corretto"),
        std::path::PathBuf::from(r"C:\Program Files\BellSoft"),
        std::path::PathBuf::from(r"C:\Program Files (x86)\Java"),
    ];
    // SDKMAN（用户目录，可能存在）
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".sdkman").join("candidates").join("java"));
        candidates.push(
            home.join("Library")
                .join("Java")
                .join("JavaVirtualMachines"),
        );
    }
    // 如果当前 Java 路径可解析，也加入其父目录
    if let Some(ref cur) = current_java {
        if let Some(parent) = cur.parent() {
            candidates.push(parent.to_path_buf());
            // 再往上两级（e.g. .../jdk-17.0.2/bin/java → .../jdk-17.0.2）
            if let Some(grand) = parent.parent() {
                candidates.push(grand.to_path_buf());
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut versions = Vec::new();
    for dir in &candidates {
        if !dir.exists() {
            continue;
        }
        // macOS JavaVirtualMachines -> .jdk -> Contents/Home
        if dir.ends_with("JavaVirtualMachines")
            || dir.to_string_lossy().contains("JavaVirtualMachines")
        {
            versions.extend(scan_macos_jvm_dir(dir, &mut seen, current_java.as_ref()));
        } else if dir.ends_with("opt")
            && (dir.starts_with("/opt/homebrew") || dir.starts_with("/usr/local"))
        {
            // Homebrew: openjdk@17, openjdk@11, etc.
            versions.extend(scan_homebrew_java_dir(
                dir,
                &mut seen,
                current_java.as_ref(),
            ));
        } else {
            versions.extend(scan_jdk_dir_generic(dir, &mut seen, current_java.as_ref()));
        }
    }

    // 最后，如果什么都没发现但 java 可用，至少显示当前版本
    if versions.is_empty() {
        if let Some(ref cur) = current_java {
            let v = run_cmd("java", &["-version"]).unwrap_or_default();
            versions.push(VersionInfo {
                version: extract_version_string(&v),
                path: cur.to_string_lossy().to_string(),
                is_active: true,
            });
        }
    }

    versions
}

/// 解析 `which java` + `readlink` 找到真实的 java 二进制路径
fn resolve_current_java_path() -> Option<std::path::PathBuf> {
    let output = run_cmd("which", &["java"])?;
    let path_str = output.trim();
    if path_str.is_empty() {
        return None;
    }
    let mut path = std::path::PathBuf::from(path_str);
    // 解析 symlink 链到真实路径
    for _ in 0..8 {
        if let Ok(target) = std::fs::read_link(&path) {
            let target = if target.is_relative() {
                if let Some(parent) = path.parent() {
                    parent.join(target)
                } else {
                    target
                }
            } else {
                target
            };
            path = target;
        } else {
            break;
        }
    }
    Some(path)
}

/// 扫描 macOS `/Library/Java/JavaVirtualMachines/xxx.jdk/Contents/Home`
fn scan_macos_jvm_dir(
    dir: &std::path::Path,
    seen: &mut std::collections::HashSet<String>,
    current_java: Option<&std::path::PathBuf>,
) -> Vec<VersionInfo> {
    let mut versions = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return versions;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let home = entry.path().join("Contents").join("Home");
        let bin_path = home.join("bin").join("java");
        if bin_path.exists() {
            let key = bin_path.to_string_lossy().to_string();
            if !seen.insert(key.clone()) {
                continue;
            }
            let v = extract_java_version_from_dir(&name);
            versions.push(VersionInfo {
                version: v,
                path: key,
                is_active: matches_current_bin(&bin_path, current_java),
            });
        }
    }
    versions
}

/// 扫描 Homebrew 安装的 openjdk（openjdk@17, openjdk@11, ...）
fn scan_homebrew_java_dir(
    dir: &std::path::Path,
    seen: &mut std::collections::HashSet<String>,
    current_java: Option<&std::path::PathBuf>,
) -> Vec<VersionInfo> {
    let mut versions = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return versions;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("openjdk") {
            continue;
        }
        let bin_path = entry.path().join("bin").join("java");
        if bin_path.exists() {
            let key = bin_path.to_string_lossy().to_string();
            if !seen.insert(key.clone()) {
                continue;
            }
            // openjdk@17 -> 17
            let v = name.trim_start_matches("openjdk@").to_string();
            let v = if v.is_empty() {
                run_cmd(&bin_path.to_string_lossy(), &["-version"])
                    .map(|s| extract_version_string(&s))
                    .unwrap_or_else(|| name.clone())
            } else {
                v
            };
            versions.push(VersionInfo {
                version: v,
                path: key,
                is_active: matches_current_bin(&bin_path, current_java),
            });
        }
    }
    versions
}

/// 通用扫描：读取 dir 下的子目录，查找 bin/java
fn scan_jdk_dir_generic(
    dir: &std::path::Path,
    seen: &mut std::collections::HashSet<String>,
    current_java: Option<&std::path::PathBuf>,
) -> Vec<VersionInfo> {
    let mut versions = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return versions;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let bin_path = entry.path().join("bin").join("java");
        if !bin_path.exists() {
            // SDKMAN 的子目录可能没有直接 bin/java（如 sdkman 下是版本目录）
            // 尝试直接找子目录下的 bin/java
            if entry.path().is_dir() {
                if let Ok(sub) = std::fs::read_dir(entry.path()) {
                    for sub_entry in sub.flatten() {
                        let sub_bin = sub_entry.path().join("bin").join("java");
                        if sub_bin.exists() {
                            let sub_name = sub_entry.file_name().to_string_lossy().to_string();
                            let key = sub_bin.to_string_lossy().to_string();
                            if !seen.insert(key.clone()) {
                                continue;
                            }
                            let v = extract_java_version_from_dir(&sub_name);
                            versions.push(VersionInfo {
                                version: v,
                                path: key,
                                is_active: matches_current_bin(&sub_bin, current_java),
                            });
                        }
                    }
                }
            }
            continue;
        }
        let key = bin_path.to_string_lossy().to_string();
        if !seen.insert(key.clone()) {
            continue;
        }
        let v = extract_java_version_from_dir(&name);
        versions.push(VersionInfo {
            version: v,
            path: key,
            is_active: matches_current_bin(&bin_path, current_java),
        });
    }
    versions
}

/// SDKMAN 专用扫描（子目录都是版本名，直接检查 bin/java）
fn scan_jdk_dir(dir: &std::path::Path, _is_sdkman: bool) -> Vec<VersionInfo> {
    let current_java = resolve_current_java_path();
    let mut versions = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return versions;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // SDKMAN 下是版本目录（如 17.0.9-tem, 11.0.21-tem）
        let bin_path = entry.path().join("bin").join("java");
        if bin_path.exists() {
            versions.push(VersionInfo {
                version: name.clone(),
                path: bin_path.to_string_lossy().to_string(),
                is_active: matches_current_bin(&bin_path, current_java.as_ref()),
            });
        }
    }
    versions
}

/// 判断 bin_path 是否与当前 java 命令对应
fn matches_current_bin(
    bin_path: &std::path::Path,
    current_java: Option<&std::path::PathBuf>,
) -> bool {
    if let Some(cur) = current_java {
        // 规范化再比较
        if let Ok(canon_cur) = cur.canonicalize() {
            if let Ok(canon_bin) = bin_path.canonicalize() {
                return canon_cur == canon_bin;
            }
        }
    }
    false
}

/// 从 java -version 输出中提取版本号
fn extract_version_string(output: &str) -> String {
    // "openjdk version \"17.0.9\" 2023-10-17" -> "17.0.9"
    // "java version \"1.8.0_202\"" -> "1.8.0_202"
    for line in output.lines() {
        if let Some(idx) = line.find("version") {
            let rest = &line[idx + 7..];
            let v = rest
                .trim()
                .trim_start_matches('"')
                .trim_start_matches('=')
                .trim_start_matches('"')
                .trim_end_matches('"');
            let v: String = v
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '_')
                .collect();
            if !v.is_empty() {
                return v;
            }
        }
    }
    output.lines().next().unwrap_or(output).to_string()
}

fn extract_java_version_from_dir(dir_name: &str) -> String {
    // e.g. "java-11-openjdk-amd64" -> "11", "jdk1.8.0_202" -> "1.8", "11.0.2" -> "11.0.2"
    // "jdk-17.0.9+9" -> "17.0.9", "zulu17.50.19-ca-jdk17.0.9" -> "17.0.9"
    let v = dir_name
        .trim_start_matches("java-")
        .trim_start_matches("jdk")
        .trim_start_matches("zulu")
        .replace("-openjdk", "")
        .replace("-oracle", "")
        .replace("-temurin", "")
        .replace("-amd64", "")
        .replace("-arm64", "")
        .replace("-ca", "")
        .replace("-fx", "");
    // take only the version-like part
    let v: String = v
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '_' || *c == '+')
        .collect();
    // strip trailing +
    let v = v.trim_end_matches('+').trim_end_matches('.').to_string();
    if v.is_empty() {
        dir_name.to_string()
    } else {
        v
    }
}

fn switch_java_version(version: &str) -> Result<String, String> {
    // 1) 可选：尝试 jenv（不依赖它，只作为附加操作）
    let _ = Command::new("jenv").args(["global", version]).output();

    // 2) 复用 scan_all_jvm_dirs 的发现结果——与前端展示的版本列表完全一致
    let all = scan_all_jvm_dirs();
    let matched = all.iter().find(|v| {
        // version 可能是 "21" 或 "21.0.2"，直接比较完整字符串或取前两段
        v.version == version || v.version.starts_with(&format!("{}.", version))
    });

    let java_home = if let Some(info) = matched {
        // 从 VersionInfo.path（bin/java 的路径）回溯到 java home
        let bin_path = std::path::Path::new(&info.path);
        bin_path
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| format!("Could not resolve java home from {}", info.path))?
            .to_path_buf()
    } else {
        // fallback：走传统搜索（兼容非标准目录）
        find_java_home_for_version(version)?
    };

    // 3) 写入 shell 配置
    let marker = "# DevNexus: Java";
    let export_line = format!(
        "{}\nexport JAVA_HOME=\"{}\"\nexport PATH=\"$JAVA_HOME/bin:$PATH\"\n",
        marker,
        java_home.display()
    );
    upsert_shell_config(marker, &export_line)?;

    // 4) 也尝试 update-alternatives（如果系统有）
    let alt_path = java_home.join("bin").join("java");
    let _ = Command::new("update-alternatives")
        .args(["--set", "java", &alt_path.to_string_lossy()])
        .output();
    let _ = Command::new("update-alternatives")
        .args([
            "--set",
            "javac",
            &java_home.join("bin").join("javac").to_string_lossy(),
        ])
        .output();

    Ok(format!(
        "Java switched to {} at {}",
        version,
        java_home.display()
    ))
}

/// 在所有 JVM 安装目录中查找匹配 version 的 JAVA_HOME（即 bin/../ 目录）
fn find_java_home_for_version(version: &str) -> Result<std::path::PathBuf, String> {
    let search_dirs = build_jvm_search_dirs();
    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // 检查直接子目录下有 bin/java（标准 JDK、SDKMAN、Homebrew 通用）
            let bin_path = entry.path().join("bin").join("java");
            if bin_path.exists() && name.contains(version) {
                return Ok(entry.path());
            }
            // macOS: xxx.jdk/Contents/Home
            let mac_home = entry.path().join("Contents").join("Home");
            let mac_bin = mac_home.join("bin").join("java");
            if mac_bin.exists() && name.contains(version) {
                return Ok(mac_home);
            }
        }
    }
    Err(format!(
        "Java version {} not found in any known installation directory. \
         Please ensure the JDK is installed and accessible.",
        version
    ))
}

fn build_jvm_search_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = vec![
        // Linux
        std::path::PathBuf::from("/usr/lib/jvm"),
        std::path::PathBuf::from("/usr/java"),
        std::path::PathBuf::from("/opt/java"),
        std::path::PathBuf::from("/opt/jdk"),
        // macOS
        std::path::PathBuf::from("/Library/Java/JavaVirtualMachines"),
        // Homebrew
        std::path::PathBuf::from("/opt/homebrew/opt"),
        std::path::PathBuf::from("/usr/local/opt"),
        // Windows
        std::path::PathBuf::from(r"C:\Program Files\Java"),
        std::path::PathBuf::from(r"C:\Program Files\Eclipse Adoptium"),
        std::path::PathBuf::from(r"C:\Program Files\Amazon Corretto"),
    ];
    // SDKMAN（用户目录，可能存在）
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".sdkman").join("candidates").join("java"));
        dirs.push(
            home.join("Library")
                .join("Java")
                .join("JavaVirtualMachines"),
        );
    }
    dirs
}

// ==================== Go (gvm) ====================

fn list_go_versions() -> Vec<VersionInfo> {
    if let Some(output) = run_cmd("gvm", &["list"]) {
        let _current = run_cmd("go", &["version"])
            .unwrap_or_default()
            .to_lowercase();
        return output
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed == "gvm" || trimmed.starts_with("---") {
                    return None;
                }
                let (is_active, version) = if trimmed.starts_with(">") {
                    (true, trimmed.trim_start_matches("> ").trim())
                } else {
                    (false, trimmed)
                };
                let version = version.trim().to_string();
                if version.is_empty() {
                    return None;
                }
                Some(VersionInfo {
                    version,
                    path: String::new(),
                    is_active,
                })
            })
            .collect();
    }
    // fallback: only show current version if go is installed
    if let Some(output) = run_cmd("go", &["version"]) {
        let v = output.trim().to_string();
        return vec![VersionInfo {
            version: v,
            path: String::new(),
            is_active: true,
        }];
    }
    vec![]
}

fn switch_go_version(version: &str) -> Result<String, String> {
    let output = Command::new("gvm")
        .args(["use", version])
        .output()
        .map_err(|e| format!("Failed to run gvm: {}", e))?;
    if output.status.success() {
        Ok(format!("Go switched to {}", version))
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("gvm switch failed: {}", err))
    }
}

// ==================== Rust (rustup) ====================

fn list_rust_versions() -> Vec<VersionInfo> {
    if let Some(output) = run_cmd("rustup", &["toolchain", "list"]) {
        let current = run_cmd("rustc", &["--version"])
            .unwrap_or_default()
            .to_lowercase();
        return output
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                let (is_active, version) =
                    if trimmed.contains("(default)") || trimmed.contains("(active)") {
                        (true, trimmed.split_whitespace().next().unwrap_or(trimmed))
                    } else {
                        (false, trimmed.split_whitespace().next().unwrap_or(trimmed))
                    };
                let version = version.trim().to_string();
                if version.is_empty() {
                    return None;
                }
                let path = format!("~/.rustup/toolchains/{}/bin", version);
                Some(VersionInfo {
                    version: version.clone(),
                    path,
                    is_active: is_active || current.contains(&version),
                })
            })
            .collect();
    }
    // fallback
    if let Some(output) = run_cmd("rustc", &["--version"]) {
        let v = output.trim().to_string();
        return vec![VersionInfo {
            version: v,
            path: String::new(),
            is_active: true,
        }];
    }
    vec![]
}

fn switch_rust_version(version: &str) -> Result<String, String> {
    let output = Command::new("rustup")
        .args(["default", version])
        .output()
        .map_err(|e| format!("Failed to run rustup: {}", e))?;
    if output.status.success() {
        Ok(format!("Rust switched to {}", version))
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("rustup switch failed: {}", err))
    }
}

// ==================== C/C++ (gcc/clang multi-version) ====================

fn list_cpp_versions() -> Vec<VersionInfo> {
    #[cfg(unix)]
    {
        let mut versions: Vec<VersionInfo> = Vec::new();
        let current_gcc = run_cmd("gcc", &["--version"])
            .unwrap_or_default()
            .to_lowercase();

        for pattern in &[
            "/usr/bin/gcc-*",
            "/usr/bin/g++-*",
            "/usr/bin/clang-*",
            "/usr/bin/clang++-*",
        ] {
            if let Ok(entries) = glob(pattern) {
                for path in entries {
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let v: String = name.chars().skip_while(|c| !c.is_ascii_digit()).collect();
                    if v.is_empty() {
                        continue;
                    }
                    versions.push(VersionInfo {
                        version: format!("{} {}", name.trim_end_matches(&v), v),
                        path,
                        is_active: current_gcc.contains(&v),
                    });
                }
            }
        }
        versions
    }
    #[cfg(not(unix))]
    {
        vec![]
    }
}

#[cfg(unix)]
fn glob(pattern: &str) -> std::io::Result<Vec<String>> {
    let mut results = Vec::new();
    let home = std::path::Path::new("/usr/bin");
    if !home.exists() {
        return Ok(results);
    }
    let prefix = pattern.trim_end_matches('*');
    if let Ok(entries) = std::fs::read_dir(
        std::path::Path::new(prefix)
            .parent()
            .unwrap_or(std::path::Path::new("/")),
    ) {
        for entry in entries.flatten() {
            let p = entry.path();
            let p_str = p.to_string_lossy().to_string();
            if p_str.starts_with(prefix.trim_end_matches('*')) && p.is_file() {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = p.metadata() {
                    if meta.permissions().mode() & 0o111 != 0 {
                        results.push(p_str);
                    }
                }
            }
        }
    }
    Ok(results)
}

fn switch_cpp_version(version: &str) -> Result<String, String> {
    // parse tool and version number from "gcc 11" or "gcc-11"
    let parts: Vec<&str> = version.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Invalid version format".to_string());
    }
    let tool = parts[0];
    let ver = if parts.len() > 1 { parts[1] } else { "" };
    if ver.is_empty() {
        return Err("Version number missing".to_string());
    }

    // Try update-alternatives on Linux
    let output = Command::new("update-alternatives")
        .args(["--set", tool, &format!("/usr/bin/{}-{}", tool, ver)])
        .output();
    if let Ok(o) = output {
        if o.status.success() {
            return Ok(format!("{} switched to version {}", tool, ver));
        }
    }
    // Fallback: create symlink
    let bin_path = format!("/usr/bin/{}-{}", tool, ver);
    if std::path::Path::new(&bin_path).exists() {
        let symlink_path = format!("/usr/local/bin/{}", tool);
        let _ = std::fs::remove_file(&symlink_path);
        #[cfg(unix)]
        if symlink(&bin_path, &symlink_path).is_ok() {
            return Ok(format!("{} switched to {} via symlink", tool, ver));
        }
        return Err(format!("Failed to create symlink for {}", tool));
    }
    Err(format!(
        "{} version {} not found at {}",
        tool, ver, bin_path
    ))
}

// ==================== 辅助函数 ====================

/// 向 shell 配置文件中写入/更新配置块。
/// 如果已存在以 `marker` 开头的内容块，则替换它；否则追加。
/// 自动检测用户所用的 shell（$SHELL），优先写入对应的配置文件。
fn upsert_shell_config(marker: &str, content: &str) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;

    // 收集所有可能存在的 shell 配置文件，全部写入（而非只写第一个）
    let shell = std::env::var("SHELL").unwrap_or_default();
    let candidate_files: Vec<String> = if cfg!(target_os = "macos") {
        vec![
            ".zshrc".to_string(),
            ".bash_profile".to_string(),
            ".bashrc".to_string(),
            ".profile".to_string(),
        ]
    } else {
        // Linux：检测实际 shell，把它对应的配置放在最前面
        let mut files = Vec::new();
        if shell.ends_with("/zsh") {
            files.push(".zshrc".to_string());
            files.push(".bashrc".to_string());
            files.push(".profile".to_string());
        } else if shell.ends_with("/bash") {
            files.push(".bashrc".to_string());
            files.push(".profile".to_string());
            files.push(".zshrc".to_string());
        } else if shell.ends_with("/fish") {
            files.push(".config/fish/config.fish".to_string());
            files.push(".bashrc".to_string());
            files.push(".profile".to_string());
        } else {
            files.push(".bashrc".to_string());
            files.push(".profile".to_string());
            files.push(".zshrc".to_string());
        }
        files
    };

    // 找所有实际存在的文件
    let mut existing_files: Vec<String> = candidate_files
        .into_iter()
        .map(|f| format!("{}/{}", home, f))
        .filter(|p| std::path::Path::new(p).exists())
        .collect();

    // 如果没有任何配置文件存在，默认用 .profile
    if existing_files.is_empty() {
        existing_files.push(format!("{}/.profile", home));
    }

    for rc_path in &existing_files {
        let existing = std::fs::read_to_string(rc_path).unwrap_or_default();
        let updated = replace_section(&existing, marker, content);
        std::fs::write(rc_path, updated).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 在文件内容中查找以 `marker` 开头的块并替换为新内容。
/// 如果没找到，则在末尾追加。
fn replace_section(existing: &str, marker: &str, new_content: &str) -> String {
    let marker_line = marker.trim();
    let lines: Vec<&str> = existing.lines().collect();
    // 查找 marker 行的位置
    if let Some(start) = lines.iter().position(|l| l.trim() == marker_line) {
        // 找到块的结束位置：从 start 往下，直到空行、下一个注释行或末尾
        let end = lines[start + 1..]
            .iter()
            .position(|l| l.trim().is_empty() || l.trim().starts_with('#'))
            .map(|pos| start + 1 + pos)
            .unwrap_or(lines.len());
        // 替换 [start, end) 区间为新内容
        let mut result: Vec<&str> = lines[..start].to_vec();
        for line in new_content.lines() {
            result.push(line);
        }
        result.extend_from_slice(&lines[end..]);
        result.join("\n")
    } else {
        // 没找到，追加
        format!("{}\n{}", existing.trim_end(), new_content)
    }
}
