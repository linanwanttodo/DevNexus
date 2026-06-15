use serde::{Deserialize, Serialize};
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
                MirrorSource {
                    name: "npmmirror (China)".into(),
                    url: "https://registry.npmmirror.com".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/npm/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tencent (China)".into(),
                    url: "https://mirrors.cloud.tencent.com/npm/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Huawei (China)".into(),
                    url: "https://mirrors.huaweicloud.com/repository/npm/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Europe (Germany)".into(),
                    url: "https://registry.npmjs.eu/".into(),
                    country: "EU".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Australia".into(),
                    url: "https://registry.npmjs.org.au/".into(),
                    country: "AU".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://registry.npmjs.org".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "pypi".into(),
            label: "PyPI".into(),
            icon: "python".into(),
            current_url: get_pypi_index(),
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://pypi.tuna.tsinghua.edu.cn/simple".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://mirrors.aliyun.com/pypi/simple/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tencent (China)".into(),
                    url: "https://mirrors.cloud.tencent.com/pypi/simple/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Huawei (China)".into(),
                    url: "https://mirrors.huaweicloud.com/repository/pypi/simple/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://pypi.org/simple/".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Google Cloud (Global)".into(),
                    url: "https://pypi-google-cloud-mirror.example.com/simple".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "docker".into(),
            label: "Docker Hub".into(),
            icon: "docker".into(),
            current_url: get_docker_mirror(),
            mirrors: vec![
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://registry.cn-hangzhou.aliyuncs.com".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tencent (China)".into(),
                    url: "https://mirror.ccs.tencentyun.com".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Docker CN".into(),
                    url: "https://registry.docker-cn.com".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "DaoCloud (China)".into(),
                    url: "https://docker.m.daocloud.io".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://docker.mirrors.tuna.tsinghua.edu.cn/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "https://docker.mirrors.ustc.edu.cn/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "NJU (China)".into(),
                    url: "https://docker.nju.edu.cn/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Google (Global)".into(),
                    url: "https://mirror.gcr.io".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "AWS (Global)".into(),
                    url: "https://public.ecr.aws".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://registry-1.docker.io".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "cargo".into(),
            label: "Cargo (Rust)".into(),
            icon: "crate".into(),
            current_url: get_cargo_mirror(),
            mirrors: vec![
                MirrorSource {
                    name: "Tuna (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "https://mirrors.ustc.edu.cn/crates.io-index/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "RsProxy (China)".into(),
                    url: "https://rsproxy.cn".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "SJTU (China)".into(),
                    url: "https://mirrors.sjtug.sjtu.edu.cn/git/crates.io-index".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://mirrors.aliyun.com/crates.io-index/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://static.rust-lang.org".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "GitHub (Global)".into(),
                    url: "https://github.com/rust-lang/crates.io-index".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        #[cfg(target_os = "macos")]
        MirrorGroup {
            id: "brew".into(),
            label: "Homebrew".into(),
            icon: "brew".into(),
            current_url: get_brew_mirror(),
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/homebrew/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "https://mirrors.ustc.edu.cn/homebrew/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://github.com/Homebrew/brew".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "composer".into(),
            label: "Composer (PHP)".into(),
            icon: "php".into(),
            current_url: get_composer_mirror(),
            mirrors: vec![
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://mirrors.aliyun.com/composer/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tencent (China)".into(),
                    url: "https://mirrors.cloud.tencent.com/composer/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://repo.packagist.org".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "go".into(),
            label: "Go Modules".into(),
            icon: "go".into(),
            current_url: get_go_proxy(),
            mirrors: vec![
                MirrorSource {
                    name: "Qiniu (goproxy.cn)".into(),
                    url: "https://goproxy.cn".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/goproxy/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://mirrors.aliyun.com/goproxy/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "https://mirrors.ustc.edu.cn/goproxy/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://proxy.golang.org".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "goproxy.io (Global)".into(),
                    url: "https://goproxy.io".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "gems".into(),
            label: "RubyGems".into(),
            icon: "ruby".into(),
            current_url: get_gems_mirror(),
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/rubygems/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "https://mirrors.ustc.edu.cn/rubygems/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://mirrors.aliyun.com/rubygems/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://rubygems.org".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "maven".into(),
            label: "Maven (Java)".into(),
            icon: "java".into(),
            current_url: get_maven_mirror(),
            mirrors: vec![
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://maven.aliyun.com/repository/public/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tencent (China)".into(),
                    url: "https://mirrors.cloud.tencent.com/maven/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/maven/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Huawei (China)".into(),
                    url: "https://mirrors.huaweicloud.com/repository/maven/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://repo.maven.apache.org/maven2".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Google Cloud (Global)".into(),
                    url: "https://storage-download.googleapis.com/maven-central".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "conda".into(),
            label: "Conda (Anaconda)".into(),
            icon: "python".into(),
            current_url: get_conda_mirror(),
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/anaconda/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "https://mirrors.ustc.edu.cn/anaconda/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://mirrors.aliyun.com/anaconda/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://repo.anaconda.com".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "nuget".into(),
            label: "NuGet (.NET)".into(),
            icon: "nuget".into(),
            current_url: get_nuget_mirror(),
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/nuget/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://mirrors.aliyun.com/nuget/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://api.nuget.org/v3/index.json".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "pub".into(),
            label: "Flutter (Pub)".into(),
            icon: "flutter".into(),
            current_url: get_pub_mirror(),
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/dart-pub/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "https://mirrors.ustc.edu.cn/dart-pub/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://pub.dev".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
    ]
}

#[tauri::command]
pub async fn test_mirror_latency(url: String) -> i64 {
    {
        if let Ok(cache) = LATENCY_CACHE.read() {
            if let Some(&(latency, cached_at)) = cache.get(&url) {
                if cached_at.elapsed().as_secs() < 60 {
                    return latency;
                }
            }
        } else {
            eprintln!("[DevNexus] Warning: Failed to acquire LATENCY_CACHE read lock");
        }
    }

    let start = Instant::now();
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("DevNexus/2.0")
        .build()
    {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let status_ok = |s: u16| matches!(s, 200..=299 | 401 | 403 | 405);
    let latency = match client.get(&url).send().await {
        Ok(resp) if status_ok(resp.status().as_u16()) => {
            let ms = start.elapsed().as_millis() as i64;
            if ms <= 0 {
                1
            } else {
                ms
            }
        }
        _ => 0,
    };

    if let Ok(mut cache) = LATENCY_CACHE.write() {
        cache.insert(url, (latency, Instant::now()));
    }

    latency
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
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }

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
        Ok(format!(
            "Homebrew mirror set to {}\n(added to {}, restart shell or source it)",
            url,
            shell_rc.file_name().unwrap_or_default().to_string_lossy()
        ))
    } else {
        // 替换已有行
        let updated = existing
            .lines()
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
        Ok(format!(
            "Homebrew mirror updated to {} in {}",
            url,
            shell_rc.file_name().unwrap_or_default().to_string_lossy()
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
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
    let shell_rc = if PathBuf::from(&home).join(".zshrc").exists() {
        PathBuf::from(&home).join(".zshrc")
    } else {
        PathBuf::from(&home).join(".bashrc")
    };
    let export_line = format!("\nexport GOPROXY=\"{},direct\"\n", url);
    let existing = fs::read_to_string(&shell_rc).unwrap_or_default();
    if !existing.contains("GOPROXY") {
        fs::write(&shell_rc, format!("{}{}", existing, export_line))
            .map_err(|e| format!("Failed to write {}: {}", shell_rc.display(), e))?;
        Ok(format!(
            "Go proxy set to {}\n(added to {}, restart shell or source it)",
            url,
            shell_rc.file_name().unwrap_or_default().to_string_lossy()
        ))
    } else {
        let updated = existing
            .lines()
            .map(|line| {
                if line.contains("GOPROXY") {
                    export_line.trim().to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&shell_rc, updated)
            .map_err(|e| format!("Failed to write {}: {}", shell_rc.display(), e))?;
        Ok(format!(
            "Go proxy updated to {} in {}",
            url,
            shell_rc.file_name().unwrap_or_default().to_string_lossy()
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
    let home = user_home();
    if home.as_os_str().is_empty() {
        return Err("Cannot determine user home directory".to_string());
    }
    let shell_rc = if PathBuf::from(&home).join(".zshrc").exists() {
        PathBuf::from(&home).join(".zshrc")
    } else {
        PathBuf::from(&home).join(".bashrc")
    };
    let export_line = format!("\nexport PUB_HOSTED_URL=\"{}\"\n", url);
    let existing = fs::read_to_string(&shell_rc).unwrap_or_default();
    if !existing.contains("PUB_HOSTED_URL") {
        fs::write(&shell_rc, format!("{}{}", existing, export_line))
            .map_err(|e| format!("Failed to write {}: {}", shell_rc.display(), e))?;
        Ok(format!(
            "Flutter Pub mirror set to {}\n(added to {}, restart shell or source it)",
            url,
            shell_rc.file_name().unwrap_or_default().to_string_lossy()
        ))
    } else {
        let updated = existing
            .lines()
            .map(|line| {
                if line.contains("PUB_HOSTED_URL") {
                    export_line.trim().to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&shell_rc, updated)
            .map_err(|e| format!("Failed to write {}: {}", shell_rc.display(), e))?;
        Ok(format!(
            "Flutter Pub mirror updated to {} in {}",
            url,
            shell_rc.file_name().unwrap_or_default().to_string_lossy()
        ))
    }
}
