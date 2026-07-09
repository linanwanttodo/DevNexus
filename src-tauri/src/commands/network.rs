use serde::{Deserialize, Serialize};
use std::process::Command;

// ── DNS ──────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DnsServer {
    pub id: u32,
    pub name: String,
    pub primary: String,
    pub secondary: String,
    pub latency: Option<u64>, // ms, None = not tested
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DnsConfig {
    pub adapter: String,
    pub current_primary: String,
    pub current_secondary: String,
}

/// List available network adapters (Linux)
#[tauri::command]
pub fn get_network_adapters() -> Result<Vec<String>, String> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "NAME,TYPE", "connection", "show", "--active"])
        .output()
        .map_err(|e| format!("Failed to run nmcli: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let adapters: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .collect();

    if adapters.is_empty() {
        // Fallback: try ip link
        let output = Command::new("ip")
            .args(["-o", "link", "show"])
            .output()
            .map_err(|e| format!("Failed to run ip: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let adapters: Vec<String> = stdout
            .lines()
            .filter_map(|line| {
                if let Some((_key, value)) = line.split_once(": ") {
                    let name = value.split('@').next().unwrap_or(value);
                    if name != "lo" {
                        Some(name.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        return Ok(adapters);
    }

    Ok(adapters)
}

/// Get current DNS for an adapter
#[tauri::command]
pub fn get_current_dns(adapter: String) -> Result<DnsConfig, String> {
    // Try nmcli first
    let output = Command::new("nmcli")
        .args(["dev", "show", &adapter])
        .output()
        .map_err(|e| format!("Failed to run nmcli: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut primary = String::new();
    let mut secondary = String::new();

    for line in stdout.lines() {
        if line.contains("DNS[1]:") {
            primary = line.split_once(':').map(|x| x.1).unwrap_or("").trim().to_string();
        } else if line.contains("DNS[2]:") {
            secondary = line.split_once(':').map(|x| x.1).unwrap_or("").trim().to_string();
        }
    }

    // Fallback: read /etc/resolv.conf
    if primary.is_empty() {
        if let Ok(content) = std::fs::read_to_string("/etc/resolv.conf") {
            for line in content.lines() {
                if line.starts_with("nameserver") {
                    let dns = line.split_whitespace().nth(1).unwrap_or("");
                    if primary.is_empty() {
                        primary = dns.to_string();
                    } else if secondary.is_empty() {
                        secondary = dns.to_string();
                    }
                }
            }
        }
    }

    Ok(DnsConfig {
        adapter,
        current_primary: primary,
        current_secondary: secondary,
    })
}

/// Test DNS latency by querying a test domain
#[tauri::command]
pub fn test_dns_latency(dns_server: String) -> Result<u64, String> {
    let start = std::time::Instant::now();

    // Try dig first with timeout
    let output = Command::new("timeout")
        .args([
            "5",
            "dig",
            &format!("@{}", dns_server),
            "github.com",
            "+short",
            "+time=3",
            "+tries=1",
        ])
        .output();

    if let Ok(out) = output {
        let elapsed = start.elapsed().as_millis() as u64;
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if !stdout.trim().is_empty() {
                return Ok(elapsed);
            }
        }
    }

    // Fallback: use timeout + nslookup
    let start = std::time::Instant::now();
    let output = Command::new("timeout")
        .args(["5", "nslookup", "github.com", &dns_server])
        .output();

    let elapsed = start.elapsed().as_millis() as u64;

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("Address:") && !stdout.contains("** server can't find") {
                Ok(elapsed)
            } else {
                Err(format!("DNS {} 解析失败", dns_server))
            }
        }
        _ => Err(format!("DNS {} 连接超时", dns_server)),
    }
}

/// Set DNS for an adapter (requires sudo)
#[tauri::command]
pub fn set_dns(adapter: String, primary: String, secondary: String) -> Result<String, String> {
    // Try nmcli first
    let mut args = vec!["con", "mod", &adapter, "ipv4.dns", &primary];
    if !secondary.is_empty() {
        args.push(&secondary);
    }

    let output = Command::new("nmcli")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run nmcli: {}", e))?;

    if output.status.success() {
        // Restart connection to apply
        let _ = Command::new("nmcli").args(["con", "up", &adapter]).output();
        return Ok(format!("DNS 已更新为 {} / {}", primary, secondary));
    }

    // Check if it's a permission error
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("permission") || stderr.contains("Not authorized") {
        return Err("需要管理员权限才能修改 DNS。请在终端中运行:\nsudo nmcli con mod \"连接名\" ipv4.dns \"DNS地址\"".to_string());
    }

    Err(format!("设置 DNS 失败: {}", stderr.trim()))
}

// ── Proxy ─────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProxyConfig {
    pub http_proxy: String,
    pub https_proxy: String,
    pub all_proxy: String,
    pub no_proxy: String,
    pub enabled: bool,
}

/// Get current proxy settings
#[tauri::command]
pub fn get_system_proxy() -> Result<ProxyConfig, String> {
    Ok(ProxyConfig {
        http_proxy: std::env::var("http_proxy")
            .or_else(|_| std::env::var("HTTP_PROXY"))
            .unwrap_or_default(),
        https_proxy: std::env::var("https_proxy")
            .or_else(|_| std::env::var("HTTPS_PROXY"))
            .unwrap_or_default(),
        all_proxy: std::env::var("all_proxy")
            .or_else(|_| std::env::var("ALL_PROXY"))
            .unwrap_or_default(),
        no_proxy: std::env::var("no_proxy")
            .or_else(|_| std::env::var("NO_PROXY"))
            .unwrap_or_default(),
        enabled: std::env::var("http_proxy").is_ok() || std::env::var("HTTP_PROXY").is_ok(),
    })
}

/// Set system proxy (writes to /etc/environment or shell profile)
#[tauri::command]
pub fn set_system_proxy(
    http_proxy: String,
    https_proxy: String,
    all_proxy: String,
    no_proxy: String,
) -> Result<String, String> {
    let proxy_section = format!(
        "\n# DevNexus Proxy Settings\n\
         export http_proxy=\"{}\"\n\
         export https_proxy=\"{}\"\n\
         export all_proxy=\"{}\"\n\
         export no_proxy=\"{}\"\n\
         export HTTP_PROXY=\"{}\"\n\
         export HTTPS_PROXY=\"{}\"\n\
         export ALL_PROXY=\"{}\"\n\
         export NO_PROXY=\"{}\"\n",
        http_proxy, https_proxy, all_proxy, no_proxy, http_proxy, https_proxy, all_proxy, no_proxy,
    );

    // Write to ~/.bashrc
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let bashrc = format!("{}/.bashrc", home);

    // Remove old DevNexus proxy block if exists
    if let Ok(content) = std::fs::read_to_string(&bashrc) {
        let cleaned = content
            .lines()
            .filter(|line| !line.starts_with("# DevNexus Proxy Settings"))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(&bashrc, cleaned);
    }

    // Append new proxy settings
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&bashrc)
        .map_err(|e| format!("Failed to open .bashrc: {}", e))?;

    file.write_all(proxy_section.as_bytes())
        .map_err(|e| format!("Failed to write .bashrc: {}", e))?;

    Ok("代理设置已保存到 ~/.bashrc，请重启终端或执行 `source ~/.bashrc` 生效".to_string())
}

/// Remove system proxy
#[tauri::command]
pub fn remove_system_proxy() -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let bashrc = format!("{}/.bashrc", home);

    if let Ok(content) = std::fs::read_to_string(&bashrc) {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut skip = false;

        for line in &lines {
            if line.starts_with("# DevNexus Proxy Settings") {
                skip = true;
                continue;
            }
            if skip && line.starts_with("export ") {
                continue;
            }
            skip = false;
            result.push(*line);
        }

        std::fs::write(&bashrc, result.join("\n"))
            .map_err(|e| format!("Failed to write .bashrc: {}", e))?;
    }

    Ok("代理设置已清除，请重启终端生效".to_string())
}

// ── GitHub Acceleration ───────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GithubHostEntry {
    pub domain: String,
    pub ip: String,
    pub enabled: bool,
}

/// Get current GitHub acceleration hosts
#[tauri::command]
pub fn get_github_hosts() -> Result<Vec<GithubHostEntry>, String> {
    let content = std::fs::read_to_string("/etc/hosts")
        .map_err(|e| format!("Failed to read /etc/hosts: {}", e))?;

    let github_domains = [
        "github.com",
        "raw.githubusercontent.com",
        "gist.githubusercontent.com",
        "assets-cdn.github.com",
        "github.global.ssl.fastly.net",
    ];

    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 2 {
            let ip = parts[0];
            let domain = parts[1];
            if github_domains.contains(&domain) {
                entries.push(GithubHostEntry {
                    domain: domain.to_string(),
                    ip: ip.to_string(),
                    enabled: true,
                });
            }
        }
    }

    // Add default entries if not present
    for domain in &github_domains {
        if !entries.iter().any(|e| e.domain == *domain) {
            entries.push(GithubHostEntry {
                domain: domain.to_string(),
                ip: String::new(),
                enabled: false,
            });
        }
    }

    Ok(entries)
}

/// Set GitHub acceleration hosts
#[tauri::command]
pub fn set_github_hosts(entries: Vec<GithubHostEntry>) -> Result<String, String> {
    let content = std::fs::read_to_string("/etc/hosts")
        .map_err(|e| format!("Failed to read /etc/hosts: {}", e))?;

    // Remove old GitHub entries
    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<String> = lines
        .iter()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return true;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                !entries.iter().any(|e| e.domain == parts[1])
            } else {
                true
            }
        })
        .map(|l| l.to_string())
        .collect();

    // Add new entries
    for entry in &entries {
        if entry.enabled && !entry.ip.is_empty() {
            result.push(format!("{} {}", entry.ip, entry.domain));
        }
    }

    std::fs::write("/etc/hosts", result.join("\n"))
        .map_err(|e| format!("Failed to write /etc/hosts: {} (需要 root 权限)", e))?;

    Ok("GitHub 加速 hosts 已更新".to_string())
}

// ── URL Latency Test ──────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LatencyResult {
    pub url: String,
    pub latency_ms: Option<u64>,
    pub status: String,
}

/// Test latency to a URL
#[tauri::command]
pub fn test_url_latency(url: String) -> Result<LatencyResult, String> {
    let start = std::time::Instant::now();

    let output = Command::new("timeout")
        .args([
            "10",
            "curl",
            "-o",
            "/dev/null",
            "-s",
            "-w",
            "%{http_code}",
            "--connect-timeout",
            "5",
            "--max-time",
            "8",
            &url,
        ])
        .output()
        .map_err(|e| format!("Failed to run curl: {}", e))?;

    let elapsed = start.elapsed().as_millis() as u64;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let status_code = stdout.trim().to_string();

    if status_code.starts_with('2') || status_code.starts_with('3') {
        Ok(LatencyResult {
            url,
            latency_ms: Some(elapsed),
            status: status_code,
        })
    } else {
        Ok(LatencyResult {
            url,
            latency_ms: None,
            status: format!("HTTP {}", status_code),
        })
    }
}

// ── Predefined DNS Servers ────────────────────────────────

#[tauri::command]
pub fn get_dns_servers() -> Vec<DnsServer> {
    vec![
        DnsServer {
            id: 1,
            name: "114DNS".into(),
            primary: "114.114.114.114".into(),
            secondary: "114.114.115.115".into(),
            latency: None,
        },
        DnsServer {
            id: 2,
            name: "DNSPod DNS+".into(),
            primary: "119.29.29.29".into(),
            secondary: "182.254.116.116".into(),
            latency: None,
        },
        DnsServer {
            id: 3,
            name: "DNS 派 电信/移动/铁通".into(),
            primary: "101.226.4.6".into(),
            secondary: "218.30.118.6".into(),
            latency: None,
        },
        DnsServer {
            id: 4,
            name: "DNS 派 联通".into(),
            primary: "123.125.81.6".into(),
            secondary: "140.207.198.6".into(),
            latency: None,
        },
        DnsServer {
            id: 5,
            name: "CNNIC DNS".into(),
            primary: "1.2.4.8".into(),
            secondary: "210.2.4.8".into(),
            latency: None,
        },
        DnsServer {
            id: 6,
            name: "Google DNS".into(),
            primary: "8.8.8.8".into(),
            secondary: "8.8.4.4".into(),
            latency: None,
        },
        DnsServer {
            id: 7,
            name: "Cloudflare DNS".into(),
            primary: "1.1.1.1".into(),
            secondary: "1.0.0.1".into(),
            latency: None,
        },
        DnsServer {
            id: 8,
            name: "IBM Quad9 DNS".into(),
            primary: "9.9.9.9".into(),
            secondary: "149.112.112.112".into(),
            latency: None,
        },
        DnsServer {
            id: 9,
            name: "DNS.SB".into(),
            primary: "185.222.222.222".into(),
            secondary: "185.184.222.222".into(),
            latency: None,
        },
        DnsServer {
            id: 10,
            name: "OpenDNS".into(),
            primary: "208.67.222.222".into(),
            secondary: "208.67.220.220".into(),
            latency: None,
        },
        DnsServer {
            id: 11,
            name: "阿里云 DNS".into(),
            primary: "223.5.5.5".into(),
            secondary: "223.6.6.6".into(),
            latency: None,
        },
        DnsServer {
            id: 12,
            name: "腾讯云 DNS".into(),
            primary: "183.60.83.19".into(),
            secondary: "183.60.82.98".into(),
            latency: None,
        },
        DnsServer {
            id: 13,
            name: "百度云 DNS".into(),
            primary: "180.76.76.76".into(),
            secondary: "1.1.1.1".into(),
            latency: None,
        },
        DnsServer {
            id: 14,
            name: "微软云 DNS".into(),
            primary: "4.2.2.1".into(),
            secondary: "4.2.2.2".into(),
            latency: None,
        },
    ]
}
