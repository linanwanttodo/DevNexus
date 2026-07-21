use dirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubMirror {
    pub name: String,
    pub url_prefix: String,
    pub enabled: bool,
    /// true: Xget 风格 — 去掉原始域名，只追加路径
    /// false: ghproxy 风格 — 完整追加原始 URL
    pub strip_host: bool,
}

impl Default for GithubMirror {
    fn default() -> Self {
        Self {
            name: "ghproxy.com".into(),
            url_prefix: "https://ghproxy.com/".into(),
            enabled: true,
            strip_host: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub max_concurrent_tasks: usize,
    pub max_chunks_per_task: usize,
    pub min_chunk_size: u64,
    pub max_chunk_size: u64,
    pub default_save_path: String,
    pub retry_count: u32,
    pub timeout_secs: u64,
    pub enable_resume: bool,
    pub github_mirrors: Vec<GithubMirror>,
    pub auto_mirror_github: bool,
    pub cookie_string: String,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        let downloads_dir = dirs::download_dir().unwrap_or_else(|| {
            dirs::home_dir().map(|h| h.join("Downloads")).unwrap_or_else(|| PathBuf::from("."))
        });

        Self {
            max_concurrent_tasks: 3,
            max_chunks_per_task: 16,
            min_chunk_size: 1_000_000,
            max_chunk_size: 100_000_000,
            default_save_path: downloads_dir.to_string_lossy().to_string(),
            retry_count: 3,
            timeout_secs: 300,
            enable_resume: true,
            github_mirrors: vec![
                GithubMirror {
                    name: "ghproxy.com".into(),
                    url_prefix: "https://ghproxy.com/".into(),
                    enabled: false,
                    strip_host: false,
                },
                GithubMirror {
                    name: "Xget".into(),
                    url_prefix: "https://xget.xi-xu.me/gh".into(),
                    enabled: true,
                    strip_host: true,
                },
                GithubMirror {
                    name: "mirror.ghproxy.com".into(),
                    url_prefix: "https://mirror.ghproxy.com/".into(),
                    enabled: false,
                    strip_host: false,
                },
            ],
            auto_mirror_github: true,
            cookie_string: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let config = DownloadConfig::default();
        assert_eq!(config.max_concurrent_tasks, 3);
        assert_eq!(config.max_chunks_per_task, 16);
        assert_eq!(config.min_chunk_size, 1_000_000);
        assert_eq!(config.max_chunk_size, 100_000_000);
        assert_eq!(config.retry_count, 3);
        assert_eq!(config.timeout_secs, 300);
        assert!(config.enable_resume);
        assert!(!config.default_save_path.is_empty());
        assert!(config.auto_mirror_github);
        assert!(!config.github_mirrors.is_empty());
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = DownloadConfig {
            max_concurrent_tasks: 5,
            max_chunks_per_task: 16,
            min_chunk_size: 2_000_000,
            max_chunk_size: 200_000_000,
            default_save_path: "/tmp/downloads".into(),
            retry_count: 5,
            timeout_secs: 600,
            enable_resume: false,
            github_mirrors: vec![GithubMirror {
                name: "test-mirror".into(),
                url_prefix: "https://test.com/".into(),
                enabled: true,
                strip_host: false,
            }],
            auto_mirror_github: false,
            cookie_string: String::new(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: DownloadConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.max_concurrent_tasks, 5);
        assert_eq!(deserialized.max_chunks_per_task, 16);
        assert_eq!(deserialized.min_chunk_size, 2_000_000);
        assert!(!deserialized.enable_resume);
        assert!(!deserialized.auto_mirror_github);
        assert_eq!(deserialized.github_mirrors.len(), 1);
        assert_eq!(deserialized.github_mirrors[0].name, "test-mirror");
        assert_eq!(deserialized.default_save_path, "/tmp/downloads");
    }

    #[test]
    fn test_github_mirror_default() {
        let mirror = GithubMirror::default();
        assert_eq!(mirror.name, "ghproxy.com");
        assert_eq!(mirror.url_prefix, "https://ghproxy.com/");
        assert!(mirror.enabled);
        assert!(!mirror.strip_host);
    }

    #[test]
    fn test_xget_mirror_strip_host() {
        let mirrors = DownloadConfig::default().github_mirrors;
        let xget = mirrors.iter().find(|m| m.name == "Xget").unwrap();
        assert!(xget.strip_host);
        assert_eq!(xget.url_prefix, "https://xget.xi-xu.me/gh");
    }
}