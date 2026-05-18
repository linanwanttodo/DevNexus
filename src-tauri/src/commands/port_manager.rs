use serde::Serialize;
use std::process::Command;

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
    let output = Command::new("lsof")
        .args(["-i", "-P", "-n", "-sTCP:LISTEN"])
        .output()
        .map_err(|e| format!("Failed to run lsof: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

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

        // 解析地址列 (如 "localhost:3000" 或 "*:8080")
        let addr = parts[8];
        if let Some(port_str) = addr.split(':').last() {
            if let Ok(port) = port_str.parse::<u16>() {
                // 去重
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

    entries.sort_by_key(|e| e.port);
    Ok(entries)
}

#[cfg(target_os = "windows")]
fn list_ports_impl() -> Result<Vec<PortEntry>, String> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .map_err(|e| format!("Failed to run netstat: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    for line in stdout.lines().skip(4) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
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
