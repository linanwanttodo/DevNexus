use serde::{Deserialize, Serialize};
use std::time::Instant;
use std::fs;
use std::path::PathBuf;

fn user_home() -> PathBuf {
    if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_default()
    } else {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MirrorSource {
    pub name: String,
    pub url: String,
    pub country: String,
    pub latency_ms: i64, // -1=未测, 0=超时/错误, >0=实际延迟(ms)
    pub is_active: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MirrorGroup {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub current_url: Option<String>,
    pub mirrors: Vec<MirrorSource>,
}

#[tauri::command]
pub fn list_mirrors() -> Vec<MirrorGroup> {
    vec![
        MirrorGroup {
            id: "npm".into(),
            label: "NPM Registry".into(),
            icon: "npm".into(),
            current_url: get_npm_registry(),
            mirrors: vec![
                MirrorSource { name: "npmmirror (China)".into(), url: "https://registry.npmmirror.com".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Tencent (China)".into(), url: "https://mirrors.cloud.tencent.com/npm/".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Huawei (China)".into(), url: "https://mirrors.huaweicloud.com/repository/npm/".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://registry.npmjs.org".into(), country: "US".into(), latency_ms: -1, is_active: false },
            ],
        },
        MirrorGroup {
            id: "pypi".into(),
            label: "PyPI".into(),
            icon: "python".into(),
            current_url: get_pypi_index(),
            mirrors: vec![
                MirrorSource { name: "Tsinghua (China)".into(), url: "https://pypi.tuna.tsinghua.edu.cn/simple".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Aliyun (China)".into(), url: "https://mirrors.aliyun.com/pypi/simple/".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Tencent (China)".into(), url: "https://mirrors.cloud.tencent.com/pypi/simple/".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://pypi.org/simple/".into(), country: "US".into(), latency_ms: -1, is_active: false },
            ],
        },
        MirrorGroup {
            id: "docker".into(),
            label: "Docker Hub".into(),
            icon: "docker".into(),
            current_url: get_docker_mirror(),
            mirrors: vec![
                MirrorSource { name: "Aliyun (China)".into(), url: "https://registry.cn-hangzhou.aliyuncs.com".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Tencent (China)".into(), url: "https://mirror.ccs.tencentyun.com".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Docker CN".into(), url: "https://registry.docker-cn.com".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "DaoCloud (China)".into(), url: "https://docker.m.daocloud.io".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://registry-1.docker.io".into(), country: "US".into(), latency_ms: -1, is_active: false },
            ],
        },
        MirrorGroup {
            id: "cargo".into(),
            label: "Cargo (Rust)".into(),
            icon: "crate".into(),
            current_url: get_cargo_mirror(),
            mirrors: vec![
                MirrorSource { name: "Tuna (China)".into(), url: "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "USTC (China)".into(), url: "https://mirrors.ustc.edu.cn/crates.io-index/".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "RsProxy (China)".into(), url: "https://rsproxy.cn".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://static.rust-lang.org".into(), country: "US".into(), latency_ms: -1, is_active: false },
            ],
        },
        #[cfg(target_os = "macos")]
        MirrorGroup {
            id: "brew".into(),
            label: "Homebrew".into(),
            icon: "brew".into(),
            current_url: get_brew_mirror(),
            mirrors: vec![
                MirrorSource { name: "Tsinghua (China)".into(), url: "https://mirrors.tuna.tsinghua.edu.cn/homebrew/".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "USTC (China)".into(), url: "https://mirrors.ustc.edu.cn/homebrew/".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://github.com/Homebrew/brew".into(), country: "US".into(), latency_ms: -1, is_active: false },
            ],
        },
        MirrorGroup {
            id: "composer".into(),
            label: "Composer (PHP)".into(),
            icon: "php".into(),
            current_url: get_composer_mirror(),
            mirrors: vec![
                MirrorSource { name: "Aliyun (China)".into(), url: "https://mirrors.aliyun.com/composer/".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Tencent (China)".into(), url: "https://mirrors.cloud.tencent.com/composer/".into(), country: "CN".into(), latency_ms: -1, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://repo.packagist.org".into(), country: "US".into(), latency_ms: -1, is_active: false },
            ],
        },
    ]
}

#[tauri::command]
pub async fn test_mirror_latency(url: String) -> i64 {
    let start = Instant::now();
    // 必须构建带超时的 Client，不用 unwrap_or_default 否则无超时
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("DevNexus/2.0")
        .build()
    {
        Ok(c) => c,
        Err(_) => return 0,
    };

    // 很多镜像站不支持 HEAD（返回 403/405），改用 GET
    // 只请求首字节即断开，避免下载整个页面
    let status_ok = |s: u16| matches!(s, 200..=299 | 401 | 403 | 405);
    match client.get(&url).send().await {
        Ok(resp) if status_ok(resp.status().as_u16()) => {
            let ms = start.elapsed().as_millis() as i64;
            if ms <= 0 { 1 } else { ms }
        }
        _ => 0,
    }
}

#[tauri::command]
pub fn switch_mirror(mirror_id: String, url: String) -> Result<String, String> {
    match mirror_id.as_str() {
        "npm" => set_npm_registry(&url),
        "pypi" => set_pypi_index(&url),
        "docker" => set_docker_mirror(&url),
        "cargo" => set_cargo_mirror(&url),
        #[cfg(target_os = "macos")]
        "brew" => set_brew_mirror(&url),
        "composer" => set_composer_mirror(&url),
        _ => Err(format!("Unknown mirror type: {}", mirror_id)),
    }
}

// ============ getters ============

fn get_npm_registry() -> Option<String> {
    let home = user_home();
    if home.as_os_str().is_empty() { return None; }
    let npmrc = PathBuf::from(&home).join(".npmrc");
    if let Ok(content) = fs::read_to_string(&npmrc) {
        for line in content.lines() {
            if line.starts_with("registry=") {
                return Some(line.trim_start_matches("registry=").trim().to_string());
            }
        }
    }
    None
}

fn get_pypi_index() -> Option<String> {
    let home = user_home();
    if home.as_os_str().is_empty() { return None; }
    for conf in &[".pip/pip.conf", ".config/pip/pip.conf"] {
        let path = PathBuf::from(&home).join(conf);
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                if line.trim().starts_with("index-url") {
                    if let Some(url) = line.split('=').nth(1) {
                        return Some(url.trim().to_string());
                    }
                }
            }
        }
    }
    None
}

fn get_docker_mirror() -> Option<String> {
    let home = user_home();
    if home.as_os_str().is_empty() { return None; }
    let daemon = PathBuf::from(&home).join(".docker/daemon.json");
    if let Ok(content) = fs::read_to_string(&daemon) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(mirrors) = json["registry-mirrors"].as_array() {
                return mirrors.first().and_then(|v| v.as_str().map(String::from));
            }
        }
    }
    None
}

fn get_cargo_mirror() -> Option<String> {
    let home = user_home();
    if home.as_os_str().is_empty() { return None; }
    let config = PathBuf::from(&home).join(".cargo/config.toml");
    if let Ok(content) = fs::read_to_string(&config) {
        let mut found_replace = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("replace-with") {
                found_replace = true;
            }
            if trimmed.starts_with("registry") {
                if let Some(url) = trimmed.split('=').nth(1) {
                    let url = url.trim().trim_matches('"').trim_matches('\'');
                    if !url.is_empty() && url != "crates-io" {
                        return Some(url.to_string());
                    }
                }
            }
        }
        if found_replace {
            return Some("custom".into());
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn get_brew_mirror() -> Option<String> {
    if let Ok(val) = std::env::var("HOMEBREW_BOTTLE_DOMAIN") {
        return Some(val);
    }
    None
}

fn get_composer_mirror() -> Option<String> {
    let home = user_home();
    if home.as_os_str().is_empty() { return None; }
    let config = PathBuf::from(&home).join(".composer/config.json");
    if let Ok(content) = fs::read_to_string(&config) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(url) = json["repositories"]["packagist"]["url"].as_str() {
                return Some(url.to_string());
            }
        }
    }
    None
}

// ============ setters ============

fn set_npm_registry(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() { return Err("Cannot determine user home directory".to_string()); }
    let npmrc = PathBuf::from(&home).join(".npmrc");
    fs::write(&npmrc, format!("registry={}\n", url))
        .map_err(|e| format!("Failed to write .npmrc: {}", e))?;
    Ok(format!("NPM registry set to {}", url))
}

fn set_pypi_index(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() { return Err("Cannot determine user home directory".to_string()); }
    let pip_dir = PathBuf::from(&home).join(".pip");
    fs::create_dir_all(&pip_dir).map_err(|e| e.to_string())?;
    let pip_conf = pip_dir.join("pip.conf");
    fs::write(&pip_conf, format!("[global]\nindex-url = {}\n", url))
        .map_err(|e| format!("Failed to write pip.conf: {}", e))?;
    Ok(format!("PyPI index set to {}", url))
}

fn set_docker_mirror(url: &str) -> Result<String, String> {
    // 优先尝试用户级配置 (~/.docker/daemon.json)，回退到系统级
    let home = user_home();
    if home.as_os_str().is_empty() { return Err("Cannot determine user home directory".to_string()); }
    let docker_dir = PathBuf::from(&home).join(".docker");
    fs::create_dir_all(&docker_dir).map_err(|e| e.to_string())?;
    let daemon = docker_dir.join("daemon.json");
    let json = serde_json::json!({ "registry-mirrors": [url] });
    fs::write(&daemon, serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Failed to write daemon.json: {}", e))?;
    Ok(format!(
        "Docker mirror set to {}\nNote: If Docker Daemon reads from /etc/docker/daemon.json, you may need to copy this config there with sudo.",
        url
    ))
}

fn set_cargo_mirror(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() { return Err("Cannot determine user home directory".to_string()); }
    let cargo_dir = PathBuf::from(&home).join(".cargo");
    fs::create_dir_all(&cargo_dir).map_err(|e| e.to_string())?;
    let config = cargo_dir.join("config.toml");
    let content = format!(
        "[source.crates-io]\nreplace-with = 'mirror'\n\n[source.mirror]\nregistry = '{}'\n",
        url
    );
    fs::write(&config, content).map_err(|e| format!("Failed to write cargo config: {}", e))?;
    Ok(format!("Cargo mirror set to {}", url))
}

#[cfg(target_os = "macos")]
fn set_brew_mirror(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() { return Err("Cannot determine user home directory".to_string()); }

    // 写入 shell profile 使生效（而不是仅当前进程）
    let shell_rc = if PathBuf::from(&home).join(".zshrc").exists() {
        PathBuf::from(&home).join(".zshrc")
    } else {
        PathBuf::from(&home).join(".bashrc")
    };

    let export_line = format!("\nexport HOMEBREW_BOTTLE_DOMAIN=\"{}\"\n", url);
    let existing = fs::read_to_string(&shell_rc).unwrap_or_default();
    if !existing.contains("HOMEBREW_BOTTLE_DOMAIN") {
        fs::write(&shell_rc, format!("{}{}", existing, export_line))
            .map_err(|e| format!("Failed to write {}: {}", shell_rc.display(), e))?;
        Ok(format!("Homebrew mirror set to {}\n(added to {}, restart shell or source it)", url, shell_rc.file_name().unwrap_or_default().to_string_lossy()))
    } else {
        // 替换已有行
        let updated = existing.lines()
            .map(|line| {
                if line.contains("HOMEBREW_BOTTLE_DOMAIN") {
                    export_line.trim().to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&shell_rc, updated)
            .map_err(|e| format!("Failed to write {}: {}", shell_rc.display(), e))?;
        Ok(format!("Homebrew mirror updated to {} in {}", url, shell_rc.file_name().unwrap_or_default().to_string_lossy()))
    }
}

fn set_composer_mirror(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() { return Err("Cannot determine user home directory".to_string()); }
    let composer_dir = PathBuf::from(&home).join(".composer");
    fs::create_dir_all(&composer_dir).map_err(|e| e.to_string())?;
    let config = composer_dir.join("config.json");

    // 正确的 Composer 全局镜像配置格式
    let json = serde_json::json!({
        "repositories": {
            "packagist": {
                "type": "composer",
                "url": url
            }
        }
    });
    fs::write(&config, serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Failed to write composer config: {}", e))?;
    Ok(format!("Composer mirror set to {}", url))
}
