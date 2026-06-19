use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, Process, ProcessesToUpdate, Signal, System};

// ==================== 全局 System 实例 ====================

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
        let mut guard = self.inner.lock().map_err(|e| e.to_string())?;
        let sys = guard.as_mut().ok_or("System not initialized")?;
        Ok(f(sys))
    }
}

// ==================== 数据结构 ====================

#[derive(Serialize, Clone)]
pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
    pub start_time_secs: u64,
    pub elapsed_secs: u64,
    pub user: String,
}

#[derive(Serialize, Clone)]
pub struct ProcessGroup {
    pub name: String,
    pub count: usize,
    pub total_cpu: f32,
    pub total_memory_bytes: u64,
    pub earliest_start: u64,
    pub entries: Vec<ProcessEntry>,
}

#[derive(Serialize)]
pub struct ProcessSummary {
    pub groups: Vec<ProcessGroup>,
    pub total: usize,
}

// ==================== 辅助函数 ====================

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn process_name(p: &Process) -> String {
    p.name().to_string_lossy().to_string()
}

fn process_user(_p: &Process) -> String {
    // sysinfo 0.33 用户字段在跨平台环境下不稳定，暂时返回空
    String::new()
}

fn entry_from(p: &Process) -> ProcessEntry {
    let name = process_name(p);
    let now = now_secs();
    ProcessEntry {
        pid: p.pid().as_u32(),
        name,
        cpu_usage: p.cpu_usage(),
        memory_bytes: p.memory(),
        start_time_secs: p.start_time(),
        elapsed_secs: now.saturating_sub(p.start_time()),
        user: process_user(p),
    }
}

// ==================== 命令 ====================

/// 列出所有进程，按 name 分组
#[tauri::command]
pub fn list_processes() -> Result<ProcessSummary, String> {
    sys().with(|sys| {
        // 第二次 refresh 才能拿到准确的 cpu_usage（首次为 0）
        sys.refresh_processes(ProcessesToUpdate::All, true);
        sys.refresh_processes(ProcessesToUpdate::All, false);

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
                });

            group.count += 1;
            group.total_cpu += entry.cpu_usage;
            group.total_memory_bytes += entry.memory_bytes;
            if entry.start_time_secs < group.earliest_start {
                group.earliest_start = entry.start_time_secs;
            }
            group.entries.push(entry);
        }

        let mut groups: Vec<ProcessGroup> = groups_map.into_values().collect();
        // 按内存降序排序（默认视图）
        groups.sort_by(|a, b| {
            b.total_memory_bytes
                .partial_cmp(&a.total_memory_bytes)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total = groups.iter().map(|g| g.count).sum();

        ProcessSummary { groups, total }
    })
}

/// 普通终止进程（SIGTERM）
#[tauri::command]
pub fn kill_process(pid: u32) -> Result<String, String> {
    sys().with(|sys| {
        let pid_key = Pid::from(pid as usize);
        let proc_ = sys
            .process(pid_key)
            .ok_or_else(|| format!("Process {} not found", pid))?;
        let name = process_name(proc_);
        match proc_.kill_with(Signal::Term) {
            Some(true) => Ok(format!("Terminated {} (PID {})", name, pid)),
            Some(false) => Err(format!(
                "Failed to terminate {} (PID {}): permission denied or process already exited",
                name, pid
            )),
            None => Err(format!("Signal not supported for {} (PID {})", name, pid)),
        }
    })?
}

/// 强制终止进程（SIGKILL）
#[tauri::command]
pub fn kill_process_force(pid: u32) -> Result<String, String> {
    sys().with(|sys| {
        let pid_key = Pid::from(pid as usize);
        let proc_ = sys
            .process(pid_key)
            .ok_or_else(|| format!("Process {} not found", pid))?;
        let name = process_name(proc_);
        match proc_.kill_with(Signal::Kill) {
            Some(true) => Ok(format!("Force killed {} (PID {})", name, pid)),
            Some(false) => Err(format!(
                "Failed to force kill {} (PID {}): permission denied or process already exited",
                name, pid
            )),
            None => Err(format!("Signal not supported for {} (PID {})", name, pid)),
        }
    })?
}
