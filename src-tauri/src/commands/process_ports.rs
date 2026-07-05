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
    info.find("(\"").and_then(|start| {
        info[start + 2..]
            .find('"')
            .map(|end| info[start + 2..start + 2 + end].to_string())
    })
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

/// Build a PID -> ports mapping from port list
fn build_port_map() -> HashMap<u32, Vec<u16>> {
    list_ports_impl()
        .unwrap_or_default()
        .into_iter()
        .fold(HashMap::new(), |mut map, entry| {
            map.entry(entry.pid)
                .or_insert_with(Vec::new)
                .push(entry.port);
            map
        })
}

// ==================== Tauri Commands ====================

/// List all processes (grouped by name) with their associated ports
#[tauri::command]
pub fn list_processes() -> Result<ProcessSummary, String> {
    sys().with(|sys| {
        sys.refresh_processes(ProcessesToUpdate::All, true);

        // Refresh CPU at most every 500ms
        static LAST_CPU: OnceLock<std::sync::Mutex<u64>> = OnceLock::new();
        let last_cpu = LAST_CPU.get_or_init(|| std::sync::Mutex::new(0));
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        {
            let mut last = last_cpu.lock().unwrap();
            if now_ms - *last > 500 {
                sys.refresh_cpu_usage();
                *last = now_ms;
            }
        }

        // Build port map
        let port_map = build_port_map();

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

        ProcessSummary { groups, total }
    })
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
