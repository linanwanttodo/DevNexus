use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Clone)]
pub struct Environment {
    pub name: String,
    pub version: String,
    pub path: String,
    pub status: String,
    pub shell_config: Option<String>, // 存储配置文件路径
}

/// 执行命令获取版本信息
fn get_version(cmd: &str, args: &[&str]) -> String {
    match Command::new(cmd).args(args).output() {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                // 提取版本号（通常在第一行）
                version.lines().next().unwrap_or("unknown").to_string()
            } else {
                "unknown".to_string()
            }
        }
        Err(_) => "not found".to_string(),
    }
}

/// 查找命令的完整路径（含 nvm 等常见路径）
fn find_cmd_path(cmd: &str) -> Option<String> {
    if let Ok(p) = which::which(cmd) {
        return Some(p.to_string_lossy().to_string());
    }

    #[cfg(unix)]
    {
        if cmd == "node" || cmd == "npm" || cmd == "npx" {
            if let Ok(home) = std::env::var("HOME") {
                let nvm_base = format!("{}/.nvm/versions/node", home);
                if let Ok(entries) = std::fs::read_dir(&nvm_base) {
                    for entry in entries.flatten() {
                        let bin = entry.path().join("bin").join(cmd);
                        if bin.exists() {
                            return Some(bin.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        let common = [
            format!("/usr/local/bin/{}", cmd),
            format!("/opt/homebrew/bin/{}", cmd),
            format!("/snap/bin/{}", cmd),
        ];
        for p in &common {
            if std::path::Path::new(p).exists() {
                return Some(p.clone());
            }
        }
    }

    #[cfg(windows)]
    {
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            let nvm_base = format!("{}\\nvm", localappdata);
            if let Ok(entries) = std::fs::read_dir(&nvm_base) {
                for entry in entries.flatten() {
                    let bin = entry.path().join(cmd);
                    if bin.exists() {
                        return Some(bin.to_string_lossy().to_string());
                    }
                }
            }
        }
        if let Ok(programfiles) = std::env::var("ProgramFiles") {
            let common = [
                format!("{}\\{}", programfiles, cmd),
                format!("{} (x86)\\{}", std::env::var("ProgramFiles(x86)").unwrap_or_default(), cmd),
            ];
            for p in &common {
                if std::path::Path::new(p).exists() {
                    return Some(p.clone());
                }
            }
        }
    }

    None
}

fn user_home() -> String {
    if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").unwrap_or_default()
    } else {
        std::env::var("HOME").unwrap_or_default()
    }
}

/// 检测单个环境
fn detect_environment(name: &str, check_cmd: &str, version_args: &[&str], config_files: &[&str]) -> Option<Environment> {
    if let Some(path) = find_cmd_path(check_cmd) {
        
        let version = get_version(check_cmd, version_args);
        
        let home = user_home();
        let shell_config = config_files.iter()
            .find(|&file| {
                let resolved = if let Some(stripped) = file.strip_prefix("~/") {
                    format!("{}/{}", home, stripped)
                } else {
                    file.to_string()
                };
                std::path::Path::new(&resolved).exists()
            })
            .map(|s| s.to_string());
        
        Some(Environment {
            name: name.to_string(),
            version,
            path,
            status: "Active".to_string(),
            shell_config,
        })
    } else {
        None
    }
}

#[tauri::command]
pub fn list_environments() -> Vec<Environment> {
    let mut envs = Vec::new();
    
    // 检测 Python
    if let Some(env) = detect_environment("Python", "python3", &["--version"], &["~/.bashrc", "~/.zshrc", "~/.profile"]) {
        envs.push(env);
    }
    
    // 检测 Node.js
    if let Some(env) = detect_environment("Node.js", "node", &["--version"], &["~/.bashrc", "~/.zshrc", "~/.profile"]) {
        envs.push(env);
    }
    
    // 检测 Go
    if let Some(env) = detect_environment("Go", "go", &["version"], &["~/.bashrc", "~/.zshrc", "~/.profile"]) {
        envs.push(env);
    }
    
    // 检测 Rust
    if let Some(env) = detect_environment("Rust", "rustc", &["--version"], &["~/.bashrc", "~/.zshrc", "~/.cargo/env"]) {
        envs.push(env);
    }
    
    // 检测 Ruby
    if let Some(env) = detect_environment("Ruby", "ruby", &["--version"], &["~/.bashrc", "~/.zshrc", "~/.profile"]) {
        envs.push(env);
    }
    
    // 检测 Java
    if let Some(env) = detect_environment("Java", "java", &["-version"], &["~/.bashrc", "~/.zshrc", "~/.profile"]) {
        envs.push(env);
    }
    
    // 检测 Docker
    if let Some(env) = detect_environment("Docker", "docker", &["--version"], &[]) {
        envs.push(env);
    }
    
    // 检测 Git
    if let Some(env) = detect_environment("Git", "git", &["--version"], &[]) {
        envs.push(env);
    }
    
    envs
}

/// 添加环境变量到 PATH
#[tauri::command]
pub fn add_to_path(env_name: String, path: String) -> Result<String, String> {
    add_to_path_impl(&env_name, &path)
}

/// 从 PATH 中移除环境变量
#[tauri::command]
pub fn remove_from_path(env_name: String, path: String) -> Result<String, String> {
    remove_from_path_impl(&env_name, &path)
}

// ==================== Unix (macOS / Linux) ====================

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn add_to_path_impl(env_name: &str, path: &str) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let export_line = format!("\n# DevNexus: {}\nexport PATH=\"{}:$PATH\"\n", env_name, path);

    let rc_files: &[&str] = if cfg!(target_os = "macos") {
        &[".zshrc", ".bash_profile", ".bashrc", ".profile"]
    } else {
        &[".bashrc", ".profile", ".zshrc"]
    };

    let mut written_to = Vec::new();
    for rc_file in rc_files {
        let rc_path = format!("{}/{}", home, rc_file);
        if std::path::Path::new(&rc_path).exists() {
            let existing = std::fs::read_to_string(&rc_path).unwrap_or_default();
            if existing.contains(path) {
                written_to.push(rc_path);
                continue; // 已存在，跳过
            }
            std::fs::write(&rc_path, format!("{}{}", existing, export_line))
                .map_err(|e| e.to_string())?;
            written_to.push(rc_path);
        }
    }

    if written_to.is_empty() {
        // 默认写入 .profile
        let profile = format!("{}/.profile", home);
        let existing = std::fs::read_to_string(&profile).unwrap_or_default();
        std::fs::write(&profile, format!("{}{}", existing, export_line))
            .map_err(|e| e.to_string())?;
        Ok(format!("Added {} to PATH in {}", env_name, profile))
    } else {
        Ok(format!("Added {} to PATH in {}", env_name, written_to.join(", ")))
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn remove_from_path_impl(env_name: &str, path: &str) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let rc_files = &[".zshrc", ".bash_profile", ".bashrc", ".profile"];

    for rc_file in rc_files {
        let rc_path = format!("{}/{}", home, rc_file);
        if std::path::Path::new(&rc_path).exists() {
            let content = std::fs::read_to_string(&rc_path).map_err(|e| e.to_string())?;
            let new_content: Vec<&str> = content
                .lines()
                .filter(|line| {
                    !line.contains(path) && !line.contains(&format!("DevNexus: {}", env_name))
                })
                .collect();
            std::fs::write(&rc_path, new_content.join("\n")).map_err(|e| e.to_string())?;
        }
    }

    Ok(format!("Removed {} from PATH", env_name))
}

// ==================== Windows ====================

#[cfg(target_os = "windows")]
fn add_to_path_impl(env_name: &str, path: &str) -> Result<String, String> {
    use std::process::Command;

    // 读取当前用户 PATH
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "[Environment]::GetEnvironmentVariable('PATH', 'User')"
            ),
        ])
        .output()
        .map_err(|e| format!("Failed to read PATH: {}", e))?;

    let current_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // 检查是否已存在
    let normalized_path = path.replace('/', "\\");
    if current_path.to_lowercase().contains(&normalized_path.to_lowercase()) {
        return Ok(format!("{} is already in PATH", env_name));
    }

    // 追加新路径
    let new_path = if current_path.is_empty() {
        normalized_path.clone()
    } else {
        format!("{};{}", current_path, normalized_path)
    };

    // 使用 PowerShell 设置用户级环境变量
    let set_output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "[Environment]::SetEnvironmentVariable('PATH', '{}', 'User')",
                new_path.replace('\'', "''")
            ),
        ])
        .output()
        .map_err(|e| format!("Failed to set PATH: {}", e))?;

    if set_output.status.success() {
        Ok(format!("Added {} to user PATH", env_name))
    } else {
        Err(format!(
            "Failed to update PATH: {}",
            String::from_utf8_lossy(&set_output.stderr)
        ))
    }
}

#[cfg(target_os = "windows")]
fn remove_from_path_impl(env_name: &str, path: &str) -> Result<String, String> {
    use std::process::Command;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "[Environment]::GetEnvironmentVariable('PATH', 'User')"
            ),
        ])
        .output()
        .map_err(|e| format!("Failed to read PATH: {}", e))?;

    let current_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let normalized_path = path.replace('/', "\\");

    // 移除匹配的路径片段
    let entries: Vec<&str> = current_path
        .split(';')
        .filter(|entry| {
            let trimmed = entry.trim();
            !trimmed.is_empty()
                && trimmed.to_lowercase() != normalized_path.to_lowercase()
        })
        .collect();

    let new_path = entries.join(";");

    if new_path == current_path {
        return Ok(format!("{} was not found in PATH", env_name));
    }

    let set_output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "[Environment]::SetEnvironmentVariable('PATH', '{}', 'User')",
                new_path.replace('\'', "''")
            ),
        ])
        .output()
        .map_err(|e| format!("Failed to set PATH: {}", e))?;

    if set_output.status.success() {
        Ok(format!("Removed {} from user PATH", env_name))
    } else {
        Err(format!(
            "Failed to update PATH: {}",
            String::from_utf8_lossy(&set_output.stderr)
        ))
    }
}
