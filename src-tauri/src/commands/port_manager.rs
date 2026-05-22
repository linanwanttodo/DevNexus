use serde::Serialize;
use std::process::Command;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_entry_serialization() {
        let entry = PortEntry {
            port: 8080,
            protocol: "TCP".to_string(),
            process_name: "nginx".to_string(),
            pid: 12345,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"port\":8080"));
        assert!(json.contains("\"pid\":12345"));
    }

    #[test]
    fn test_port_entry_sort_key() {
        let entries = vec![
            PortEntry { port: 443, protocol: "TCP".to_string(), process_name: "a".to_string(), pid: 1 },
            PortEntry { port: 80, protocol: "TCP".to_string(), process_name: "b".to_string(), pid: 2 },
            PortEntry { port: 8080, protocol: "TCP".to_string(), process_name: "c".to_string(), pid: 3 },
        ];
        let mut sorted = entries.clone();
        sorted.sort_by_key(|e| e.port);
        assert_eq!(sorted[0].port, 80);
        assert_eq!(sorted[1].port, 443);
        assert_eq!(sorted[2].port, 8080);
    }

    #[test]
    fn test_extract_ss_process_name_normal() {
        let info = r#"users:(("nginx",pid=12345,fd=3))"#;
        let name = extract_ss_process_name(info);
        assert_eq!(name, Some("nginx".to_string()));
    }

    #[test]
    fn test_extract_ss_process_name_no_match() {
        assert_eq!(extract_ss_process_name("no quotes here"), None);
        assert_eq!(extract_ss_process_name(""), None);
    }

    #[test]
    fn test_extract_ss_pid_normal() {
        let info = r#"users:(("nginx",pid=12345,fd=3))"#;
        let pid = extract_ss_pid(info);
        assert_eq!(pid, Some(12345));
    }

    #[test]
    fn test_extract_ss_pid_no_match() {
        assert_eq!(extract_ss_pid("no pid here"), None);
        assert_eq!(extract_ss_pid(""), None);
        assert_eq!(extract_ss_pid("pid=abc"), None); // non-numeric
    }
}

#[derive(Serialize, Clone)]
pub struct PortEntry {
    pub port: u16,
    pub protocol: String,
    pub process_name: String,
    pub pid: u32,
}

/// 列出所有监听端口
#[tauri::command]
pub fn list_ports() -> Result<Vec<PortEntry>, String> {
    list_ports_impl()
}

/// 结束占用指定端口的进程
#[tauri::command]
pub fn kill_port(port: u16) -> Result<String, String> {
    let entries = list_ports_impl()?;
    let target = entries
        .iter()
        .find(|e| e.port == port)
        .ok_or_else(|| format!("No process found on port {}", port))?;

    kill_process(target.pid)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn list_ports_impl() -> Result<Vec<PortEntry>, String> {
    // 优先用 lsof，如果不存在则用 ss（更快且无需安装）
    let lsof_result = Command::new("lsof")
        .args(["-i", "-P", "-n", "-sTCP:LISTEN"])
        .output();

    let output = match lsof_result {
        Ok(o) if o.status.success() => o,
        _ => {
            // lsof 不存在或失败，回退到 ss
            Command::new("ss")
                .args(["-tlnp"])
                .output()
                .map_err(|e| format!("Neither lsof nor ss are available: {}", e))?
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    // 判断是 lsof 还是 ss 输出
    let is_ss = stdout.lines().next().map(|l| l.contains("State") || l.contains("Recv-Q")).unwrap_or(false);

    if is_ss {
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 6 {
                continue;
            }

            let local_addr = parts[3];
            let port = local_addr.rsplit(':').next().and_then(|p| p.parse::<u16>().ok());
            let Some(port) = port else { continue };

            let process_info = parts[5..].join(" ");
            let process_name = extract_ss_process_name(&process_info).unwrap_or_else(|| "unknown".to_string());
            let pid = extract_ss_pid(&process_info).unwrap_or(0);
            if pid == 0 {
                continue;
            }

            if !entries.iter().any(|e: &PortEntry| e.port == port && e.pid == pid) {
                entries.push(PortEntry {
                    port,
                    protocol: "TCP".to_string(),
                    process_name,
                    pid,
                });
            }
        }
    } else {
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 9 {
                continue;
            }

            let process_name = parts[0].to_string();
            let pid: u32 = parts[1].parse().unwrap_or(0);
            if pid == 0 {
                continue;
            }

            let addr = parts[8];
            if let Some(port_str) = addr.split(':').next_back() {
                if let Ok(port) = port_str.parse::<u16>() {
                    if !entries.iter().any(|e: &PortEntry| e.port == port && e.pid == pid) {
                        entries.push(PortEntry {
                            port,
                            protocol: "TCP".to_string(),
                            process_name,
                            pid,
                        });
                    }
                }
            }
        }
    }

    entries.sort_by_key(|e| e.port);
    Ok(entries)
}

fn extract_ss_process_name(info: &str) -> Option<String> {
    info.find("(\"").and_then(|start| {
        info[start + 2..].find('"').map(|end| info[start + 2..start + 2 + end].to_string())
    })
}

fn extract_ss_pid(info: &str) -> Option<u32> {
    info.find("pid=").and_then(|start| {
        let rest = &info[start + 4..];
        rest.find(',').or_else(|| rest.find(')')).and_then(|end| rest[..end].parse::<u32>().ok())
    })
}

#[cfg(target_os = "windows")]
fn list_ports_impl() -> Result<Vec<PortEntry>, String> {
    let output = Command::new("netstat")
        .args(["-ano"])
        .output()
        .map_err(|e| format!("Failed to run netstat: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in stdout.lines().skip(3) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        let proto = parts[0].to_uppercase();
        if !proto.starts_with("TCP") {
            continue;
        }

        let state = parts[3];
        if state != "LISTENING" {
            continue;
        }

        let local_addr = parts[1];
        let pid: u32 = parts[4].parse().unwrap_or(0);
        if pid == 0 {
            continue;
        }

        if let Some(port_str) = local_addr.rsplit(':').next() {
            if let Ok(port) = port_str.parse::<u16>() {
                if !entries.iter().any(|e: &PortEntry| e.port == port && e.pid == pid) {
                    entries.push(PortEntry {
                        port,
                        protocol: "TCP".to_string(),
                        process_name: format!("PID:{}", pid),
                        pid,
                    });
                }
            }
        }
    }

    entries.sort_by_key(|e| e.port);
    Ok(entries)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn kill_process(pid: u32) -> Result<String, String> {
    let output = Command::new("kill")
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("Failed to kill process {}: {}", pid, e))?;

    if output.status.success() {
        Ok(format!("Process {} terminated", pid))
    } else {
        // 尝试强制杀死
        let output = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
            .map_err(|e| format!("Failed to force kill {}: {}", pid, e))?;

        if output.status.success() {
            Ok(format!("Process {} force-killed", pid))
        } else {
            Err(format!("Failed to kill process {}", pid))
        }
    }
}

#[cfg(target_os = "windows")]
fn kill_process(pid: u32) -> Result<String, String> {
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .map_err(|e| format!("Failed to kill process {}: {}", pid, e))?;

    if output.status.success() {
        Ok(format!("Process {} terminated", pid))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
