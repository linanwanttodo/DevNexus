use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, ProcessesToUpdate, Signal, System};

// ==================== Global System instance ====================

fn sys() -> &'static Mutex {
    static SYS: OnceLock<Mutex> = OnceLock::new();
    SYS.get_or_init(|| Mutex::new(System::new_all()))
}

struct Mutex {
    inner: std::sync::Mutex<Option<System>>,
}

impl Mutex {
    fn new(sys: System) -> Self {
        Self {
            inner: std::sync::Mutex::new(Some(sys)),
        }
    }

    fn with<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut System) -> R,
    {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        let sys = guard.as_mut().ok_or("System not initialized")?;
        Ok(f(sys))
    }
}

// ==================== Data structures ====================

#[derive(Serialize, Clone)]
pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
    pub start_time_secs: u64,
    pub elapsed_secs: u64,
}

#[derive(Serialize, Clone)]
pub struct ProcessGroup {
    pub name: String,
    pub count: usize,
    pub total_cpu: f32,
    pub total_memory_bytes: u64,
    pub earliest_start: u64,
    pub entries: Vec<ProcessEntry>,
    pub ports: Vec<u16>,
}

#[derive(Serialize)]
pub struct ProcessSummary {
    pub groups: Vec<ProcessGroup>,
    pub total: usize,
}

#[derive(Serialize, Clone)]
pub struct PortEntry {
    pub port: u16,
    pub protocol: String,
    pub process_name: String,
    pub pid: u32,
}

// ==================== Process helpers ====================

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn process_name(p: &sysinfo::Process) -> String {
    p.name().to_string_lossy().to_string()
}

fn entry_from(p: &sysinfo::Process) -> ProcessEntry {
    let name = process_name(p);
    let now = now_secs();
    ProcessEntry {
        pid: p.pid().as_u32(),
        name,
        cpu_usage: p.cpu_usage(),
        memory_bytes: p.memory(),
        start_time_secs: p.start_time(),
        elapsed_secs: now.saturating_sub(p.start_time()),
    }
}

// ==================== Port helpers (platform-specific) ====================

fn list_ports_impl() -> Result<Vec<PortEntry>, String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        list_ports_unix()
    }
    #[cfg(target_os = "windows")]
    {
        list_ports_windows()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err("Unsupported platform".to_string())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn list_ports_unix() -> Result<Vec<PortEntry>, String> {
    use std::process::Command;

    let lsof_result = Command::new("lsof")
        .args(["-i", "-P", "-n", "-sTCP:LISTEN"])
        .output();

    let output = match lsof_result {
        Ok(o) if o.status.success() => o,
        _ => Command::new("ss")
            .args(["-tlnp"])
            .output()
            .map_err(|e| format!("Neither lsof nor ss available: {}", e))?,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();

    let is_ss = stdout
        .lines()
        .next()
        .map(|l| l.contains("State") || l.contains("Recv-Q"))
        .unwrap_or(false);

    if is_ss {
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 6 {
                continue;
            }

            let port = parts[3]
                .rsplit(':')
                .next()
                .and_then(|p| p.parse::<u16>().ok());
            let Some(port) = port else { continue };

            let info = parts[5..].join(" ");
            let pid = extract_ss_pid(&info).unwrap_or(0);
            if pid == 0 {
                continue;
            }

            let process_name =
                extract_ss_process_name(&info).unwrap_or_else(|| "unknown".to_string());

            if !entries
                .iter()
                .any(|e: &PortEntry| e.port == port && e.pid == pid)
            {
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

            if let Some(port_str) = parts[8].split(':').next_back() {
                if let Ok(port) = port_str.parse::<u16>() {
                    if !entries
                        .iter()
                        .any(|e: &PortEntry| e.port == port && e.pid == pid)
                    {
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn extract_ss_process_name(info: &str) -> Option<String> {
    info.find("(\"")
        .and_then(|start| {
            info[start + 2..]
                .find('"')
                .map(|end| info[start + 2..start + 2 + end].to_string())
        })
        // 空进程名视为解析失败，让调用方回退到 "unknown"
        .filter(|name| !name.is_empty())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn extract_ss_pid(info: &str) -> Option<u32> {
    info.find("pid=").and_then(|start| {
        let rest = &info[start + 4..];
        rest.find(',')
            .or_else(|| rest.find(')'))
            .and_then(|end| rest[..end].parse::<u32>().ok())
    })
}

#[cfg(target_os = "windows")]
fn list_ports_windows() -> Result<Vec<PortEntry>, String> {
    use std::process::Command;

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
        if parts[3] != "LISTENING" {
            continue;
        }

        let pid: u32 = parts[4].parse().unwrap_or(0);
        if pid == 0 {
            continue;
        }

        if let Some(port_str) = parts[1].rsplit(':').next() {
            if let Ok(port) = port_str.parse::<u16>() {
                if !entries
                    .iter()
                    .any(|e: &PortEntry| e.port == port && e.pid == pid)
                {
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

/// Build a PID -> ports mapping from port list (cached 2s to avoid repeated lsof/ss forks)
#[allow(clippy::type_complexity)]
fn build_port_map() -> HashMap<u32, Vec<u16>> {
    static CACHE: OnceLock<std::sync::Mutex<Option<(std::time::Instant, HashMap<u32, Vec<u16>>)>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((at, ref map)) = *guard {
            if at.elapsed().as_secs() < 2 {
                return map.clone();
            }
        }
    }
    let map =
        list_ports_impl()
            .unwrap_or_default()
            .into_iter()
            .fold(HashMap::new(), |mut map, entry| {
                map.entry(entry.pid)
                    .or_insert_with(Vec::new)
                    .push(entry.port);
                map
            });
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((std::time::Instant::now(), map.clone()));
    }
    map
}

// ==================== Tauri Commands ====================

/// List all processes (grouped by name) with their associated ports
#[tauri::command]
pub fn list_processes() -> Result<ProcessSummary, String> {
    // 子进程调用（lsof/ss/netstat）在锁外完成，避免长时间 I/O 占用全局锁
    let port_map = build_port_map();

    sys().with(|sys| -> Result<ProcessSummary, String> {
        sys.refresh_processes(ProcessesToUpdate::All, true);

        // Refresh CPU at most every 500ms
        static LAST_CPU: OnceLock<std::sync::Mutex<u64>> = OnceLock::new();
        let last_cpu = LAST_CPU.get_or_init(|| std::sync::Mutex::new(0));
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        {
            let mut last = match last_cpu.lock() {
                Ok(guard) => guard,
                Err(_) => return Err("internal lock poisoned".to_string()),
            };
            if now_ms.saturating_sub(*last) > 500 {
                sys.refresh_cpu_usage();
                *last = now_ms;
            }
        }

        let now = now_secs();
        let mut groups_map: HashMap<String, ProcessGroup> = HashMap::new();

        for proc_ in sys.processes().values() {
            let name = process_name(proc_);
            if name.is_empty() {
                continue;
            }

            let entry = entry_from(proc_);

            let group = groups_map
                .entry(name.clone())
                .or_insert_with(|| ProcessGroup {
                    name,
                    count: 0,
                    total_cpu: 0.0,
                    total_memory_bytes: 0,
                    earliest_start: now,
                    entries: Vec::new(),
                    ports: Vec::new(),
                });

            group.count += 1;
            group.total_cpu += entry.cpu_usage;
            group.total_memory_bytes += entry.memory_bytes;
            if entry.start_time_secs < group.earliest_start {
                group.earliest_start = entry.start_time_secs;
            }
            group.entries.push(entry);

            // Merge ports for this PID
            if let Some(ports) = port_map.get(&proc_.pid().as_u32()) {
                for &p in ports {
                    if !group.ports.contains(&p) {
                        group.ports.push(p);
                    }
                }
            }
        }

        // Sort ports within each group
        for g in groups_map.values_mut() {
            g.ports.sort();
        }

        let mut groups: Vec<ProcessGroup> = groups_map.into_values().collect();
        groups.sort_by(|a, b| {
            b.total_memory_bytes
                .partial_cmp(&a.total_memory_bytes)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total = groups.iter().map(|g| g.count).sum();

        Ok(ProcessSummary { groups, total })
    })?
}

/// List all listening ports
#[tauri::command]
pub fn list_ports() -> Result<Vec<PortEntry>, String> {
    list_ports_impl()
}

/// Kill a process by PID (SIGTERM, then SIGKILL if needed)
#[tauri::command]
pub fn kill_process(pid: u32) -> Result<String, String> {
    sys().with(|sys| {
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let pid_key = Pid::from(pid as usize);
        let proc_ = sys
            .process(pid_key)
            .ok_or_else(|| format!("Process {} not found", pid))?;
        let name = process_name(proc_);

        // Try SIGTERM first
        if let Some(true) = proc_.kill_with(Signal::Term) {
            return Ok(format!("Terminated {} (PID {})", name, pid));
        }
        // Fallback to SIGKILL
        match proc_.kill_with(Signal::Kill) {
            Some(true) => Ok(format!("Force killed {} (PID {})", name, pid)),
            Some(false) => Err(format!(
                "Failed to kill {} (PID {}): permission denied",
                name, pid
            )),
            None => Err(format!("Signal not supported for {} (PID {})", name, pid)),
        }
    })?
}

/// Force kill a process by PID (SIGKILL)
#[tauri::command]
pub fn kill_process_force(pid: u32) -> Result<String, String> {
    sys().with(|sys| {
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let pid_key = Pid::from(pid as usize);
        let proc_ = sys
            .process(pid_key)
            .ok_or_else(|| format!("Process {} not found", pid))?;
        let name = process_name(proc_);
        match proc_.kill_with(Signal::Kill) {
            Some(true) => Ok(format!("Force killed {} (PID {})", name, pid)),
            Some(false) => Err(format!(
                "Failed to force kill {} (PID {}): permission denied",
                name, pid
            )),
            None => Err(format!("Signal not supported for {} (PID {})", name, pid)),
        }
    })?
}

/// Kill the process on a specific port
#[tauri::command]
pub fn kill_port(port: u16) -> Result<String, String> {
    let entries = list_ports_impl()?;
    let target = entries
        .iter()
        .find(|e| e.port == port)
        .ok_or_else(|| format!("No process found on port {}", port))?;
    // Reuse sysinfo-based kill
    kill_process_force(target.pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    mod ss_parsing {
        use super::{extract_ss_pid, extract_ss_process_name};

        #[test]
        fn test_extract_ss_pid_typical() {
            // ss -tlnp 的典型输出: users:(("nginx",pid=1234,fd=10))
            assert_eq!(
                extract_ss_pid("users:((\"nginx\",pid=1234,fd=10))"),
                Some(1234)
            );
        }

        #[test]
        fn test_extract_ss_pid_without_fd() {
            assert_eq!(extract_ss_pid("users:((\"node\",pid=5678))"), Some(5678));
        }

        #[test]
        fn test_extract_ss_pid_missing() {
            assert_eq!(extract_ss_pid("users:((\"systemd\"))"), None);
            assert_eq!(extract_ss_pid(""), None);
            assert_eq!(extract_ss_pid("pid=abc"), None);
        }

        #[test]
        fn test_extract_ss_pid_after_fd() {
            // fd 出现在 pid 前面时也能正确解析
            assert_eq!(
                extract_ss_pid("users:((\"python\",fd=5,pid=999))"),
                Some(999)
            );
        }

        #[test]
        fn test_extract_ss_process_name_typical() {
            assert_eq!(
                extract_ss_process_name("users:((\"nginx\",pid=1234,fd=10))"),
                Some("nginx".to_string())
            );
        }

        #[test]
        fn test_extract_ss_process_name_missing() {
            assert_eq!(extract_ss_process_name("users:((\"\"))"), None);
            assert_eq!(extract_ss_process_name("no users field"), None);
        }

        #[test]
        fn test_extract_ss_process_name_multiple_entries() {
            // 多个进程共享端口: users:(("nginx",pid=1,fd=10),("nginx",pid=2,fd=11))
            assert_eq!(
                extract_ss_process_name("users:((\"nginx\",pid=1,fd=10),(\"nginx\",pid=2,fd=11))"),
                Some("nginx".to_string())
            );
        }
    }

    #[test]
    fn test_kill_port_no_process_error_path() {
        // 端口 0 永远不会被 LISTEN（保留端口），且不依赖真实环境：
        // 只要返回 Err（找不到进程）或 Err（lsof/ss 不可用）都属于正确错误路径，
        // 绝不应 Ok —— 这验证 kill_port 在目标缺失时不会误杀。
        let result = kill_port(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_entry_defaults_serializable() {
        // ProcessEntry / ProcessGroup / PortEntry 必须可序列化（IPC 传输）
        let entry = ProcessEntry {
            pid: 42,
            name: "test".to_string(),
            cpu_usage: 0.5,
            memory_bytes: 1024,
            start_time_secs: 100,
            elapsed_secs: 10,
        };
        let json = serde_json::to_string(&entry).expect("serialize ProcessEntry");
        assert!(json.contains("\"pid\":42"));
        assert!(json.contains("\"name\":\"test\""));

        let group = ProcessGroup {
            name: "test".to_string(),
            count: 1,
            total_cpu: 0.5,
            total_memory_bytes: 1024,
            earliest_start: 100,
            entries: vec![entry],
            ports: vec![8080, 3000],
        };
        let json = serde_json::to_string(&group).expect("serialize ProcessGroup");
        assert!(json.contains("\"ports\":[3000,8080]") || json.contains("\"ports\":[8080,3000]"));

        let port = PortEntry {
            port: 8080,
            protocol: "TCP".to_string(),
            process_name: "nginx".to_string(),
            pid: 42,
        };
        let json = serde_json::to_string(&port).expect("serialize PortEntry");
        assert!(json.contains("\"port\":8080"));
    }

    #[test]
    fn test_now_secs_monotonic() {
        let a = now_secs();
        let b = now_secs();
        assert!(b >= a);
    }
}
