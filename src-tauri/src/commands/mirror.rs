use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::Instant;

/// 镜像测速缓存：URL -> (延迟ms, 缓存时间)，TTL 60 秒
/// 使用 RwLock 提升读并发性能，多线程读取时不需要互斥
static LATENCY_CACHE: std::sync::LazyLock<RwLock<HashMap<String, (i64, Instant)>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mirror_source_serialization() {
        let source = MirrorSource {
            name: "Test Mirror".to_string(),
            url: "https://example.com".to_string(),
            country: "CN".to_string(),
            latency_ms: -1,
            is_active: false,
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("Test Mirror"));
        assert!(json.contains("\"latency_ms\":-1"));
    }

    #[test]
    fn test_mirror_group_creation() {
        let mirror = MirrorSource {
            name: "Official".to_string(),
            url: "https://registry.npmjs.org".to_string(),
            country: "US".to_string(),
            latency_ms: 100,
            is_active: true,
        };
        let group = MirrorGroup {
            id: "npm".to_string(),
            label: "NPM Registry".to_string(),
            icon: "npm".to_string(),
            current_url: Some("https://registry.npmjs.org".to_string()),
            mirrors: vec![mirror],
        };
        assert_eq!(group.id, "npm");
        assert_eq!(group.mirrors.len(), 1);
        assert!(group.mirrors[0].is_active);
    }

    #[test]
    fn test_list_mirrors_contains_expected_groups() {
        let groups = list_mirrors();
        let ids: Vec<&str> = groups.iter().map(|g| g.id.as_str()).collect();
        assert!(ids.contains(&"npm"));
        assert!(ids.contains(&"pypi"));
        assert!(ids.contains(&"docker"));
        assert!(ids.contains(&"cargo"));
        assert!(ids.contains(&"composer"));
        assert!(ids.contains(&"go"));
        assert!(ids.contains(&"gems"));
        assert!(ids.contains(&"maven"));
        assert!(ids.contains(&"conda"));
        assert!(ids.contains(&"nuget"));
        assert!(ids.contains(&"pub"));
    }

    #[test]
    fn test_mirror_latency_default_state() {
        let source = MirrorSource {
            name: "Unmeasured".to_string(),
            url: "https://example.com".to_string(),
            country: "US".to_string(),
            latency_ms: -1,
            is_active: false,
        };
        assert_eq!(source.latency_ms, -1);
        assert!(!source.is_active);
    }

    #[test]
    fn test_mirror_group_serialization_roundtrip() {
        let groups = list_mirrors();
        let json = serde_json::to_string(&groups).unwrap();
        let deserialized: Vec<MirrorGroup> = serde_json::from_str(&json).unwrap();
        assert_eq!(groups.len(), deserialized.len());
        assert_eq!(groups[0].id, deserialized[0].id);
    }

    #[test]
    fn test_switch_mirror_unknown_type() {
        let result = switch_mirror("unknown".to_string(), "https://example.com".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown mirror type"));
    }
}

fn user_home() -> PathBuf {
    PathBuf::from(crate::utils::user_home())
}

/// 静态镜像源数据表（镜像源名称/URL/类别/国家）已拆出到 mirror_data 模块
#[path = "mirror_data.rs"]
mod mirror_data;
pub use mirror_data::{MirrorGroup, MirrorSource};

/// List all available mirror groups with their current URLs.
///
/// Scans system configuration files to detect currently active mirror URLs
/// for each package manager (npm, pypi, docker, cargo, go, maven, gradle).
///
/// # Returns
/// A vector of `MirrorGroup` structs containing mirror information.
#[tauri::command]
pub fn list_mirrors() -> Vec<MirrorGroup> {
    // 静态数据来自 mirror_data::list_mirror_groups()，此处只填充各镜像当前生效 URL
    let mut groups = mirror_data::list_mirror_groups();
    for group in &mut groups {
        group.current_url = match group.id.as_str() {
            "npm" => get_npm_registry(),
            "pypi" => get_pypi_index(),
            "docker" => get_docker_mirror(),
            "cargo" => get_cargo_mirror(),
            #[cfg(target_os = "macos")]
            "brew" => get_brew_mirror(),
            "composer" => get_composer_mirror(),
            "go" => get_go_proxy(),
            "gems" => get_gems_mirror(),
            "maven" => get_maven_mirror(),
            "conda" => get_conda_mirror(),
            "nuget" => get_nuget_mirror(),
            "pub" => get_pub_mirror(),
            _ => None,
        };
    }
    groups
}

/// 测试镜像延迟：成功返回 Ok(延迟ms)，失败返回 Err（区分超时与其他错误）。
/// 返回类型保持数值便于前端展示，失败通过 Err 表达，不再用 0 混淆"超时/错误"与真实延迟。
#[tauri::command]
pub async fn test_mirror_latency(url: String) -> Result<i64, String> {
    {
        if let Ok(cache) = LATENCY_CACHE.read() {
            if let Some(&(latency, cached_at)) = cache.get(&url) {
                if cached_at.elapsed().as_secs() < 60 {
                    return Ok(latency);
                }
            }
        } else {
            tracing::warn!("[DevNexus] Failed to acquire latency cache read lock");
        }
    }

    let start = Instant::now();
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("DevNexus/2.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err(format!("failed to build HTTP client: {}", e)),
    };

    let status_ok = |s: u16| matches!(s, 200..=299 | 401 | 403 | 405);
    // cargo sparse 镜像带 sparse+ 协议前缀，HTTP 探测时剥掉
    let probe_url = url
        .strip_prefix("sparse+")
        .unwrap_or(url.as_str())
        .to_string();
    let latency = match client.get(&probe_url).send().await {
        Ok(resp) if status_ok(resp.status().as_u16()) => {
            let ms = start.elapsed().as_millis() as i64;
            if ms <= 0 {
                1
            } else {
                ms
            }
        }
        Ok(resp) => return Err(format!("mirror returned HTTP {}", resp.status())),
        Err(e) if e.is_timeout() => return Err("timeout".to_string()),
        Err(e) => return Err(format!("failed to reach mirror: {}", e)),
    };

    // 仅缓存成功结果，避免把错误/超时作为 0 混入缓存
    if let Ok(mut cache) = LATENCY_CACHE.write() {
        cache.insert(url, (latency, Instant::now()));
    }

    Ok(latency)
}

/// Switch to a specific mirror URL for the given package manager.
///
/// Updates the configuration file for the specified mirror type (npm, pypi, docker, cargo, go, maven, gradle)
/// to use the provided URL. On Linux/macOS, may require sudo password for system-wide changes.
///
/// # Arguments
/// * `mirror_id` - The ID of the mirror group to switch (e.g., "npm", "pypi", "docker")
/// * `url` - The new mirror URL to use
///
/// # Returns
/// A success message or error description.
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
        "go" => set_go_proxy(&url),
        "gems" => set_gems_mirror(&url),
        "maven" => set_maven_mirror(&url),
        "conda" => set_conda_mirror(&url),
        "nuget" => set_nuget_mirror(&url),
        "pub" => set_pub_mirror(&url),
        _ => Err(format!("Unknown mirror type: {}", mirror_id)),
    }
}

// ============ getters ============

fn get_npm_registry() -> Option<String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return None;
    }
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
    if home.as_os_str().is_empty() {
        return None;
    }
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
    if home.as_os_str().is_empty() {
        return None;
    }
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
    if home.as_os_str().is_empty() {
        return None;
    }
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
    if home.as_os_str().is_empty() {
        return None;
    }
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

fn get_go_proxy() -> Option<String> {
    if let Ok(val) = std::env::var("GOPROXY") {
        let first = val.split(',').next().unwrap_or("").to_string();
        if !first.is_empty() && first != "https://proxy.golang.org,direct" {
            return Some(first);
        }
    }
    None
}

fn get_gems_mirror() -> Option<String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return None;
    }
    let gemrc = PathBuf::from(&home).join(".gemrc");
    if let Ok(content) = fs::read_to_string(&gemrc) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(val) = json.get(":source").and_then(|v| v.as_str()) {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn get_maven_mirror() -> Option<String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return None;
    }
    let settings = PathBuf::from(&home).join(".m2/settings.xml");
    if let Ok(content) = fs::read_to_string(&settings) {
        // 简单解析 Maven settings.xml 中第一个 mirror 的 url
        if let Some(start) = content.find("<url>") {
            if let Some(end) = content[start + 5..].find("</url>") {
                return Some(content[start + 5..start + 5 + end].to_string());
            }
        }
    }
    None
}

fn get_conda_mirror() -> Option<String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return None;
    }
    let condarc = PathBuf::from(&home).join(".condarc");
    if let Ok(content) = fs::read_to_string(&condarc) {
        // 纯文本解析 ~/.condarc (YAML 格式)
        for line in content.lines() {
            let t = line.trim();
            if t.starts_with("channel_alias:") {
                if let Some(url) = t.split(':').nth(1) {
                    let url = url.trim();
                    if !url.is_empty() {
                        return Some(url.to_string());
                    }
                }
            }
        }
        // 找 channels 下列表中的非 defaults channel
        let mut in_channels = false;
        for line in content.lines() {
            let t = line.trim();
            if t == "channels:" {
                in_channels = true;
                continue;
            }
            if in_channels {
                if t.starts_with('-') {
                    let channel = t.trim_start_matches('-').trim();
                    if channel != "defaults" && !channel.is_empty() {
                        return Some(channel.to_string());
                    }
                } else if !t.is_empty() && !t.starts_with('#') {
                    // channels 块结束
                    break;
                }
            }
        }
    }
    None
}

fn get_nuget_mirror() -> Option<String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return None;
    }
    let config = PathBuf::from(&home).join(".nuget/NuGet/NuGet.Config");
    if let Ok(content) = fs::read_to_string(&config) {
        // 简单的 XML 解析
        if let Some(start) = content.find("<add key=\"nuget.org\" value=\"") {
            let after = &content[start + 31..];
            if let Some(end) = after.find('"') {
                return Some(after[..end].to_string());
            }
        }
    }
    None
}

fn get_pub_mirror() -> Option<String> {
    if let Ok(val) = std::env::var("PUB_HOSTED_URL") {
        if !val.is_empty() {
            return Some(val);
        }
    }
    None
}

// ============ setters ============

fn set_npm_registry(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
    let npmrc = PathBuf::from(&home).join(".npmrc");
    fs::write(&npmrc, format!("registry={}\n", url))
        .map_err(|e| format!("Failed to write .npmrc: {}", e))?;
    Ok(format!("NPM registry set to {}", url))
}

fn set_pypi_index(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
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
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
    let docker_dir = PathBuf::from(&home).join(".docker");
    fs::create_dir_all(&docker_dir).map_err(|e| e.to_string())?;
    let daemon = docker_dir.join("daemon.json");
    let json = serde_json::json!({ "registry-mirrors": [url] });
    fs::write(
        &daemon,
        serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Failed to write daemon.json: {}", e))?;
    Ok(format!(
        "Docker mirror set to {}\nNote: If Docker Daemon reads from /etc/docker/daemon.json, you may need to copy this config there with sudo.",
        url
    ))
}

fn set_cargo_mirror(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
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
    crate::utils::validate_rc_value(url)?;
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }

    // 写入 shell profile 使生效（而不是仅当前进程）
    let added = crate::utils::rc_editor::set_export_line(&home, "HOMEBREW_BOTTLE_DOMAIN", url)?;
    let rc_file = crate::utils::rc_editor::detect_shell_rc(&home);
    if added {
        Ok(format!(
            "Homebrew mirror set to {}\n(added to {}, restart shell or source it)",
            url,
            rc_file.file_name().unwrap_or_default().to_string_lossy()
        ))
    } else {
        Ok(format!(
            "Homebrew mirror updated to {} in {}",
            url,
            rc_file.file_name().unwrap_or_default().to_string_lossy()
        ))
    }
}

fn set_composer_mirror(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
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
    fs::write(
        &config,
        serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Failed to write composer config: {}", e))?;
    Ok(format!("Composer mirror set to {}", url))
}

fn set_go_proxy(url: &str) -> Result<String, String> {
    crate::utils::validate_rc_value(url)?;
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
    let added =
        crate::utils::rc_editor::set_export_line(&home, "GOPROXY", &format!("{},direct", url))?;
    let rc_file = crate::utils::rc_editor::detect_shell_rc(&home);
    if added {
        Ok(format!(
            "Go proxy set to {}\n(added to {}, restart shell or source it)",
            url,
            rc_file.file_name().unwrap_or_default().to_string_lossy()
        ))
    } else {
        Ok(format!(
            "Go proxy updated to {} in {}",
            url,
            rc_file.file_name().unwrap_or_default().to_string_lossy()
        ))
    }
}

fn set_gems_mirror(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
    let gemrc = PathBuf::from(&home).join(".gemrc");
    let json = serde_json::json!({ ":source": url });
    fs::write(
        &gemrc,
        serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Failed to write .gemrc: {}", e))?;
    Ok(format!("RubyGems mirror set to {}", url))
}

fn set_maven_mirror(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
    let m2_dir = PathBuf::from(&home).join(".m2");
    fs::create_dir_all(&m2_dir).map_err(|e| e.to_string())?;
    let settings = m2_dir.join("settings.xml");
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<settings xmlns="http://maven.apache.org/SETTINGS/1.0.0"
          xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
          xsi:schemaLocation="http://maven.apache.org/SETTINGS/1.0.0 http://maven.apache.org/xsd/settings-1.0.0.xsd">
    <mirrors>
        <mirror>
            <id>devnexus-mirror</id>
            <mirrorOf>*</mirrorOf>
            <url>{}</url>
        </mirror>
    </mirrors>
</settings>
"#,
        url
    );
    fs::write(&settings, content)
        .map_err(|e| format!("Failed to write Maven settings.xml: {}", e))?;
    Ok(format!("Maven mirror set to {}", url))
}

fn set_conda_mirror(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
    let condarc = PathBuf::from(&home).join(".condarc");
    let content = format!("channel_alias: {}\nchannels:\n  - defaults\n", url);
    fs::write(&condarc, content).map_err(|e| format!("Failed to write .condarc: {}", e))?;
    Ok(format!("Conda mirror set to {}", url))
}

fn set_nuget_mirror(url: &str) -> Result<String, String> {
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
    let nuget_dir = PathBuf::from(&home).join(".nuget/NuGet");
    fs::create_dir_all(&nuget_dir).map_err(|e| e.to_string())?;
    let config = nuget_dir.join("NuGet.Config");
    let content = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <clear />
    <add key="nuget-mirror" value="{}" />
  </packageSources>
</configuration>
"#,
        url
    );
    fs::write(&config, content).map_err(|e| format!("Failed to write NuGet.Config: {}", e))?;
    Ok(format!("NuGet mirror set to {}", url))
}

fn set_pub_mirror(url: &str) -> Result<String, String> {
    crate::utils::validate_rc_value(url)?;
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
    let added = crate::utils::rc_editor::set_export_line(&home, "PUB_HOSTED_URL", url)?;
    let rc_file = crate::utils::rc_editor::detect_shell_rc(&home);
    if added {
        Ok(format!(
            "Flutter Pub mirror set to {}\n(added to {}, restart shell or source it)",
            url,
            rc_file.file_name().unwrap_or_default().to_string_lossy()
        ))
    } else {
        Ok(format!(
            "Flutter Pub mirror updated to {} in {}",
            url,
            rc_file.file_name().unwrap_or_default().to_string_lossy()
        ))
    }
}
