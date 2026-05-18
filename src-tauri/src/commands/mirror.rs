use serde::{Deserialize, Serialize};
use std::time::Instant;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct MirrorSource {
    pub name: String,
    pub url: String,
    pub country: String,
    pub latency_ms: u64,
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
                MirrorSource { name: "npmmirror (China)".into(), url: "https://registry.npmmirror.com".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Tencent (China)".into(), url: "https://mirrors.cloud.tencent.com/npm/".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Huawei (China)".into(), url: "https://mirrors.huaweicloud.com/repository/npm/".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Yandex (Russia)".into(), url: "https://npm.yandex.ru".into(), country: "RU".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://registry.npmjs.org".into(), country: "US".into(), latency_ms: 0, is_active: false },
            ],
        },
        MirrorGroup {
            id: "pypi".into(),
            label: "PyPI".into(),
            icon: "python".into(),
            current_url: get_pypi_index(),
            mirrors: vec![
                MirrorSource { name: "Tsinghua (China)".into(), url: "https://pypi.tuna.tsinghua.edu.cn/simple".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Aliyun (China)".into(), url: "https://mirrors.aliyun.com/pypi/simple/".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Tencent (China)".into(), url: "https://mirrors.cloud.tencent.com/pypi/simple/".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Yandex (Russia)".into(), url: "https://pypi.yandex.ru/simple/".into(), country: "RU".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://pypi.org/simple/".into(), country: "US".into(), latency_ms: 0, is_active: false },
            ],
        },
        MirrorGroup {
            id: "docker".into(),
            label: "Docker Hub".into(),
            icon: "docker".into(),
            current_url: get_docker_mirror(),
            mirrors: vec![
                MirrorSource { name: "Aliyun (China)".into(), url: "https://registry.cn-hangzhou.aliyuncs.com".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Tencent (China)".into(), url: "https://mirror.ccs.tencentyun.com".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Docker CN".into(), url: "https://registry.docker-cn.com".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Yandex (Russia)".into(), url: "https://mirror.yandex.ru/mirrors/docker/".into(), country: "RU".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://registry-1.docker.io".into(), country: "US".into(), latency_ms: 0, is_active: false },
            ],
        },
        MirrorGroup {
            id: "cargo".into(),
            label: "Cargo (Rust)".into(),
            icon: "crate".into(),
            current_url: get_cargo_mirror(),
            mirrors: vec![
                MirrorSource { name: "Tuna (China)".into(), url: "https://mirrors.tuna.tsinghua.edu.cn/rustup".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "USTC (China)".into(), url: "https://mirrors.ustc.edu.cn/rust-static".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Yandex (Russia)".into(), url: "https://mirror.yandex.ru/mirrors/rust/".into(), country: "RU".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://static.rust-lang.org".into(), country: "US".into(), latency_ms: 0, is_active: false },
            ],
        },
        MirrorGroup {
            id: "brew".into(),
            label: "Homebrew".into(),
            icon: "brew".into(),
            current_url: get_brew_mirror(),
            mirrors: vec![
                MirrorSource { name: "Tsinghua (China)".into(), url: "https://mirrors.tuna.tsinghua.edu.cn/homebrew/".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "USTC (China)".into(), url: "https://mirrors.ustc.edu.cn/homebrew/".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://github.com/Homebrew/brew".into(), country: "US".into(), latency_ms: 0, is_active: false },
            ],
        },
        MirrorGroup {
            id: "composer".into(),
            label: "Composer (PHP)".into(),
            icon: "php".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource { name: "Aliyun (China)".into(), url: "https://mirrors.aliyun.com/composer/".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Tencent (China)".into(), url: "https://mirrors.cloud.tencent.com/composer/".into(), country: "CN".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Yandex (Russia)".into(), url: "https://mirror.yandex.ru/mirrors/composer/".into(), country: "RU".into(), latency_ms: 0, is_active: false },
                MirrorSource { name: "Official".into(), url: "https://repo.packagist.org".into(), country: "US".into(), latency_ms: 0, is_active: false },
            ],
        },
    ]
}

#[tauri::command]
pub async fn test_mirror_latency(url: String) -> u64 {
    let start = Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let _ = client.head(&url).send().await;
    start.elapsed().as_millis() as u64
}

#[tauri::command]
pub fn switch_mirror(mirror_id: String, url: String) -> Result<String, String> {
    match mirror_id.as_str() {
        "npm" => set_npm_registry(&url),
        "pypi" => set_pypi_index(&url),
        "docker" => set_docker_mirror(&url),
        "cargo" => set_cargo_mirror(&url),
        "brew" => set_brew_mirror(&url),
        "composer" => set_composer_mirror(&url),
        _ => Err(format!("Unknown mirror type: {}", mirror_id)),
    }
}

// ============ getters ============

fn get_npm_registry() -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
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
    let home = std::env::var("HOME").unwrap_or_default();
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
    let home = std::env::var("HOME").unwrap_or_default();
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
    let home = std::env::var("HOME").unwrap_or_default();
    let config = PathBuf::from(&home).join(".cargo/config.toml");
    if let Ok(content) = fs::read_to_string(&config) {
        for line in content.lines() {
            if line.contains("replace-with") {
                return Some("custom".into());
            }
        }
    }
    None
}

fn get_brew_mirror() -> Option<String> {
    if let Ok(val) = std::env::var("HOMEBREW_BOTTLE_DOMAIN") {
        return Some(val);
    }
    None
}

// ============ setters ============

fn set_npm_registry(url: &str) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let npmrc = PathBuf::from(&home).join(".npmrc");
    fs::write(&npmrc, format!("registry={}\n", url))
        .map_err(|e| format!("Failed to write .npmrc: {}", e))?;
    Ok(format!("NPM registry set to {}", url))
}

fn set_pypi_index(url: &str) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let pip_dir = PathBuf::from(&home).join(".pip");
    fs::create_dir_all(&pip_dir).map_err(|e| e.to_string())?;
    let pip_conf = pip_dir.join("pip.conf");
    fs::write(&pip_conf, format!("[global]\nindex-url = {}\n", url))
        .map_err(|e| format!("Failed to write pip.conf: {}", e))?;
    Ok(format!("PyPI index set to {}", url))
}

fn set_docker_mirror(url: &str) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let docker_dir = PathBuf::from(&home).join(".docker");
    fs::create_dir_all(&docker_dir).map_err(|e| e.to_string())?;
    let daemon = docker_dir.join("daemon.json");
    let json = serde_json::json!({ "registry-mirrors": [url] });
    fs::write(&daemon, serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Failed to write daemon.json: {}", e))?;
    Ok(format!("Docker mirror set to {}", url))
}

fn set_cargo_mirror(url: &str) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
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

fn set_brew_mirror(url: &str) -> Result<String, String> {
    std::env::set_var("HOMEBREW_BOTTLE_DOMAIN", url);
    Ok(format!("Homebrew mirror set to {} (session only)", url))
}

fn set_composer_mirror(url: &str) -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let composer_dir = PathBuf::from(&home).join(".composer");
    fs::create_dir_all(&composer_dir).map_err(|e| e.to_string())?;
    let config = composer_dir.join("config.json");
    let json = serde_json::json!({ "repositories": [{ "type": "composer", "url": url }] });
    fs::write(&config, serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Failed to write composer config: {}", e))?;
    Ok(format!("Composer mirror set to {}", url))
}
