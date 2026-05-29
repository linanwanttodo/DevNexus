use chrono::Local;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_type_default() {
        let tt: TaskType = Default::default();
        assert!(matches!(tt, TaskType::Shell));
    }

    #[test]
    fn test_task_status_default() {
        let ts: TaskStatus = Default::default();
        assert!(matches!(ts, TaskStatus::Idle));
    }

    #[test]
    fn test_task_type_serde_shell() {
        let json = serde_json::to_string(&TaskType::Shell).unwrap();
        assert_eq!(json, "\"shell\"");
    }

    #[test]
    fn test_task_type_serde_python() {
        let json = serde_json::to_string(&TaskType::Python).unwrap();
        assert_eq!(json, "\"python\"");
    }

    #[test]
    fn test_task_type_serde_shutdown() {
        let json = serde_json::to_string(&TaskType::Shutdown).unwrap();
        assert_eq!(json, "\"shutdown\"");
    }

    #[test]
    fn test_task_status_serde_idle() {
        let json = serde_json::to_string(&TaskStatus::Idle).unwrap();
        assert_eq!(json, "\"idle\"");
    }

    #[test]
    fn test_task_status_serde_disabled() {
        let json = serde_json::to_string(&TaskStatus::Disabled).unwrap();
        assert_eq!(json, "\"disabled\"");
    }

    #[test]
    fn test_task_log_creation() {
        let log = TaskLog {
            id: 1,
            task_id: 42,
            started_at: "2024-01-01 12:00:00".to_string(),
            finished_at: Some("2024-01-01 12:00:05".to_string()),
            status: "success".to_string(),
            exit_code: Some(0),
            stdout: "hello".to_string(),
            stderr: String::new(),
            duration_ms: Some(5000),
        };
        assert_eq!(log.id, 1);
        assert_eq!(log.status, "success");
        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains("\"exit_code\":0"));
    }

    #[test]
    fn test_scheduled_task_creation() {
        let task = ScheduledTask {
            id: 1,
            name: "Test Task".to_string(),
            cron_expression: "0 */5 * * * *".to_string(),
            task_type: TaskType::Shell,
            content: Some("echo hello".to_string()),
            status: TaskStatus::Idle,
            timeout_secs: 30,
            last_run: None,
            last_status: None,
            next_run: None,
            run_count: 0,
            created_at: "2024-01-01".to_string(),
        };
        assert_eq!(task.name, "Test Task");
        assert_eq!(task.run_count, 0);
        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"cron_expression\":\"0 */5 * * * *\""));
    }

    #[test]
    fn test_scheduled_task_serialization_roundtrip() {
        let task = ScheduledTask {
            id: 1,
            name: "Backup".to_string(),
            cron_expression: "0 0 * * * *".to_string(),
            task_type: TaskType::Python,
            content: Some("print('backup')".to_string()),
            status: TaskStatus::Disabled,
            timeout_secs: 60,
            last_run: Some("2024-01-01 00:00:00".to_string()),
            last_status: Some("success".to_string()),
            next_run: Some("2024-01-02 00:00:00".to_string()),
            run_count: 10,
            created_at: "2023-12-01".to_string(),
        };
        let json = serde_json::to_string(&task).unwrap();
        let deserialized: ScheduledTask = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, task.id);
        assert_eq!(deserialized.name, task.name);
        assert_eq!(deserialized.run_count, task.run_count);
        assert!(matches!(deserialized.task_type, TaskType::Python));
        assert!(matches!(deserialized.status, TaskStatus::Disabled));
    }

    #[test]
    fn test_task_scheduler_initial_state() {
        let s = TaskScheduler::new();
        assert!(s.tasks.lock().unwrap().is_empty());
        assert!(s.logs.lock().unwrap().is_empty());
        assert_eq!(*s.next_id.lock().unwrap(), 1);
    }
}

// ============ Data Models ============

/// 任务类型
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub enum TaskType {
    #[default]
    #[serde(rename = "shell")]
    Shell,
    #[serde(rename = "python")]
    Python,
    #[serde(rename = "shutdown")]
    Shutdown,
}

/// 任务状态
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub enum TaskStatus {
    #[default]
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "disabled")]
    Disabled,
}

/// 单次执行日志
#[derive(Serialize, Deserialize, Clone)]
pub struct TaskLog {
    pub id: u64,
    pub task_id: u32,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String, // "success", "failed", "timeout"
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: Option<u64>,
}

/// 定时任务定义（参考青龙面板设计）
#[derive(Serialize, Deserialize, Clone)]
pub struct ScheduledTask {
    pub id: u32,
    pub name: String,
    pub cron_expression: String,
    pub task_type: TaskType,     // "shell" | "python" | "shutdown"
    pub content: Option<String>, // shell/python 的脚本内容，shutdown 为 None
    pub status: TaskStatus,
    pub timeout_secs: u64, // 沙箱超时时间（秒）
    pub last_run: Option<String>,
    pub last_status: Option<String>, // "success" | "failed" | "timeout"
    pub next_run: Option<String>,
    pub run_count: u32,
    pub created_at: String,
}

/// 任务调度器状态
pub struct TaskScheduler {
    pub tasks: Arc<Mutex<Vec<ScheduledTask>>>,
    pub logs: Arc<Mutex<Vec<TaskLog>>>,
    pub next_id: Arc<Mutex<u32>>,
    pub next_log_id: Arc<Mutex<u64>>,
}

impl TaskScheduler {
    pub fn new() -> Self {
        let mut s = Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
            logs: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
            next_log_id: Arc::new(Mutex::new(1)),
        };
        s.load();
        s
    }

    fn tasks_path() -> std::path::PathBuf {
        crate::utils::data_dir().join("tasks.json")
    }

    fn logs_dir() -> std::path::PathBuf {
        crate::utils::data_dir().join("task_logs")
    }

    fn log_path(task_id: u32) -> std::path::PathBuf {
        Self::logs_dir().join(format!("task_{}.json", task_id))
    }

    // ============ 保存/加载 ============

    fn save_tasks(&self) {
        let path = Self::tasks_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(tasks) = self.tasks.lock() {
            if let Ok(json) = serde_json::to_string_pretty(&*tasks) {
                let _ = std::fs::write(&path, json);
            }
        }
    }

    fn load(&mut self) {
        // 加载任务
        let path = Self::tasks_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(tasks) = serde_json::from_str::<Vec<ScheduledTask>>(&data) {
                let max_id = tasks.iter().map(|t| t.id).max().unwrap_or(0);
                if let Ok(mut next_id) = self.next_id.lock() {
                    *next_id = max_id + 1;
                }
                if let Ok(mut t) = self.tasks.lock() {
                    *t = tasks;
                }
            }
        }

        // 加载日志
        let logs_dir = Self::logs_dir();
        if logs_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&logs_dir) {
                let mut all_logs = Vec::new();
                let mut max_log_id: u64 = 0;
                for entry in entries.flatten() {
                    if let Ok(data) = std::fs::read_to_string(entry.path()) {
                        if let Ok(logs) = serde_json::from_str::<Vec<TaskLog>>(&data) {
                            for log in &logs {
                                if log.id > max_log_id {
                                    max_log_id = log.id;
                                }
                            }
                            all_logs.extend(logs);
                        }
                    }
                }
                if let Ok(mut l) = self.logs.lock() {
                    *l = all_logs;
                }
                if let Ok(mut nid) = self.next_log_id.lock() {
                    *nid = max_log_id + 1;
                }
            }
        }
    }

    // ============ 后台调度循环 ============

    pub fn start_background(&self) {
        const CHECK_INTERVAL_SECS: u64 = 30;
        let tasks_arc = self.tasks.clone();
        let logs_arc = self.logs.clone();
        let next_log_id = self.next_log_id.clone();

        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(CHECK_INTERVAL_SECS));
            interval.tick().await;

            loop {
                interval.tick().await;
                let now = Local::now();
                let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
                let mut due: Vec<ScheduledTask> = Vec::new();

                // 收集到期任务
                {
                    let Ok(tasks) = tasks_arc.lock() else {
                        continue;
                    };
                    for t in tasks.iter() {
                        if !matches!(t.status, TaskStatus::Idle) {
                            continue;
                        }
                        let should_run = match &t.next_run {
                            Some(nr) => nr.as_str() <= now_str.as_str(),
                            None => true,
                        };
                        if should_run {
                            due.push(t.clone());
                        }
                    }
                }

                for task in &due {
                    // 创建日志条目
                    let log_id = {
                        let Ok(mut nid) = next_log_id.lock() else {
                            continue;
                        };
                        let id = *nid;
                        *nid += 1;
                        id
                    };

                    let started_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

                    let log = TaskLog {
                        id: log_id,
                        task_id: task.id,
                        started_at: started_at.clone(),
                        finished_at: None,
                        status: "running".to_string(),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        duration_ms: None,
                    };
                    {
                        let Ok(mut guard) = logs_arc.lock() else {
                            continue;
                        };
                        guard.push(log);
                    }

                    // 执行任务
                    let timeout = task.timeout_secs;
                    let result = match task.task_type {
                        TaskType::Shell => {
                            if let Some(ref c) = task.content {
                                execute_with_sandbox("sh", &["-c", c], timeout).await
                            } else {
                                Err("No script content".to_string())
                            }
                        }
                        TaskType::Python => {
                            if let Some(ref c) = task.content {
                                execute_with_sandbox("python3", &["-c", c], timeout).await
                            } else {
                                Err("No script content".to_string())
                            }
                        }
                        TaskType::Shutdown => execute_shutdown().await,
                    };

                    let start_time =
                        chrono::NaiveDateTime::parse_from_str(&started_at, "%Y-%m-%d %H:%M:%S")
                            .unwrap_or_else(|_| Local::now().naive_local());

                    // 更新日志
                    {
                        let Ok(mut all_logs) = logs_arc.lock() else {
                            continue;
                        };
                        if let Some(existing) = all_logs.iter_mut().find(|l| l.id == log_id) {
                            let now = Local::now().naive_local();
                            let duration = (now - start_time).num_milliseconds().max(0) as u64;
                            existing.finished_at =
                                Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
                            existing.duration_ms = Some(duration);
                            match &result {
                                Ok((code, stdout, stderr)) => {
                                    existing.status =
                                        if *code == 0 { "success" } else { "failed" }.to_string();
                                    existing.exit_code = Some(*code);
                                    existing.stdout = stdout.clone();
                                    existing.stderr = stderr.clone();
                                }
                                Err(err) => {
                                    existing.status = "timeout".to_string();
                                    existing.stderr = err.clone();
                                }
                            }
                        }
                    }

                    // 更新任务状态
                    {
                        let Ok(mut tasks) = tasks_arc.lock() else {
                            continue;
                        };
                        if let Some(t) = tasks.iter_mut().find(|t| t.id == task.id) {
                            t.last_run = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
                            t.run_count += 1;
                            t.last_status = Some(match &result {
                                Ok((code, _, _)) => {
                                    if *code == 0 { "success" } else { "failed" }.to_string()
                                }
                                Err(_) => "timeout".to_string(),
                            });
                            if result.is_ok() {
                                t.next_run = calculate_next_run(&t.cron_expression).ok().flatten();
                            }
                        }
                    }

                    // 持久化
                    save_tasks_impl(&tasks_arc);
                    save_logs_impl(&logs_arc, task.id);
                }
            }
        });
    }
}

fn save_tasks_impl(tasks_arc: &Arc<Mutex<Vec<ScheduledTask>>>) {
    let path = TaskScheduler::tasks_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(tasks) = tasks_arc.lock() {
        if let Ok(json) = serde_json::to_string_pretty(&*tasks) {
            let _ = std::fs::write(&path, json);
        }
    }
}

fn save_logs_impl(logs_arc: &Arc<Mutex<Vec<TaskLog>>>, task_id: u32) {
    let path = TaskScheduler::log_path(task_id);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(logs) = logs_arc.lock() {
        let task_logs: Vec<&TaskLog> = logs.iter().filter(|l| l.task_id == task_id).collect();
        if let Ok(json) = serde_json::to_string_pretty(&task_logs) {
            let _ = std::fs::write(&path, json);
        }
    }
}

// ============ 沙箱执行 ============

/// 带沙箱的执行：超时限制 + 进程组隔离
async fn execute_with_sandbox(
    program: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Result<(i32, String, String), String> {
    let program = program.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();

    tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tokio::task::spawn_blocking(move || {
            let mut cmd = Command::new(&program);
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            cmd.args(&arg_refs);

            // Linux: 使用进程组隔离
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::process::CommandExt;
                unsafe {
                    cmd.pre_exec(|| {
                        libc::setpgid(0, 0);
                        Ok(())
                    });
                }
            }

            let output = cmd
                .output()
                .map_err(|e| format!("Failed to execute {}: {}", program, e))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let code = output.status.code().unwrap_or(-1);

            Ok((code, stdout, stderr))
        }),
    )
    .await
    .map_err(|_| {
        format!(
            "Execution timed out after {}s (sandbox limit)",
            timeout_secs
        )
    })?
    .map_err(|e| format!("Task spawn error: {}", e))?
}

// ============ 跨平台关机 ============

/// 执行关机命令（跨平台）
async fn execute_shutdown() -> Result<(i32, String, String), String> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        {
            let output = Command::new("osascript")
                .args(["-e", "tell application \"Finder\" to shut down"])
                .output()
                .map_err(|e| format!("Failed to execute shutdown: {}", e))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let code = output.status.code().unwrap_or(-1);
            return Ok((code, stdout, stderr));
        }

        #[cfg(target_os = "linux")]
        {
            let output = Command::new("systemctl")
                .args(["poweroff"])
                .output()
                .map_err(|e| format!("Failed to execute systemctl: {}", e))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let code = output.status.code().unwrap_or(-1);
            Ok((code, stdout, stderr))
        }

        #[cfg(target_os = "windows")]
        {
            let output = Command::new("shutdown")
                .args(["/s", "/t", "0"])
                .output()
                .map_err(|e| format!("Failed to execute shutdown: {}", e))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let code = output.status.code().unwrap_or(-1);
            return Ok((code, stdout, stderr));
        }
    })
    .await
    .map_err(|e| format!("Task spawn error: {}", e))?
}

// ============ Tauri Commands ============

/// 创建定时任务
#[tauri::command]
pub fn add_task(
    name: String,
    cron_expression: String,
    task_type: String,
    content: Option<String>,
    timeout_secs: Option<u64>,
    state: tauri::State<'_, TaskScheduler>,
) -> Result<u32, String> {
    // 验证 cron
    if let Err(e) = cron::Schedule::try_from(cron_expression.as_str()) {
        return Err(format!("Invalid cron expression: {}", e));
    }

    // 验证任务类型
    let tt = match task_type.as_str() {
        "shell" => TaskType::Shell,
        "python" => TaskType::Python,
        "shutdown" => TaskType::Shutdown,
        _ => {
            return Err(format!(
                "Unknown task type: {}. Supported: shell, python, shutdown",
                task_type
            ))
        }
    };

    // shell/python 必须有内容
    if matches!(tt, TaskType::Shell | TaskType::Python)
        && content
            .as_ref()
            .map(|c| c.trim().is_empty())
            .unwrap_or(true)
    {
        return Err("Script content cannot be empty".to_string());
    }

    // 验证 Python 可用性
    if matches!(tt, TaskType::Python) && which::which("python3").is_err() {
        return Err("Python3 is not installed or not in PATH".to_string());
    }

    let mut next_id = state.next_id.lock().map_err(|e| e.to_string())?;
    let id = *next_id;
    *next_id += 1;
    drop(next_id);

    let task = ScheduledTask {
        id,
        name,
        cron_expression: cron_expression.clone(),
        task_type: tt,
        content,
        status: TaskStatus::Idle,
        timeout_secs: timeout_secs.unwrap_or(300), // 默认 5 分钟超时
        last_run: None,
        last_status: None,
        next_run: calculate_next_run(&cron_expression)?,
        run_count: 0,
        created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    state.tasks.lock().map_err(|e| e.to_string())?.push(task);

    state.save_tasks();

    Ok(id)
}

/// 获取所有任务
#[tauri::command]
pub fn list_tasks(state: tauri::State<'_, TaskScheduler>) -> Vec<ScheduledTask> {
    state
        .tasks
        .lock()
        .map(|tasks| tasks.clone())
        .unwrap_or_else(|_| Vec::new())
}

/// 删除任务（同时删除关联日志）
#[tauri::command]
pub fn delete_task(task_id: u32, state: tauri::State<'_, TaskScheduler>) -> Result<(), String> {
    {
        let mut tasks = state.tasks.lock().map_err(|e| e.to_string())?;
        tasks.retain(|t| t.id != task_id);
    }
    {
        let mut logs = state.logs.lock().map_err(|e| e.to_string())?;
        logs.retain(|l| l.task_id != task_id);
    }
    let log_path = TaskScheduler::log_path(task_id);
    let _ = std::fs::remove_file(&log_path);
    state.save_tasks();
    Ok(())
}

/// 启用/禁用任务
#[tauri::command]
pub fn toggle_task(task_id: u32, state: tauri::State<'_, TaskScheduler>) -> Result<(), String> {
    let mut tasks = state.tasks.lock().map_err(|e| e.to_string())?;
    if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
        task.status = match task.status {
            TaskStatus::Idle => TaskStatus::Disabled,
            TaskStatus::Disabled => TaskStatus::Idle,
        };
        state.save_tasks();
        Ok(())
    } else {
        Err("Task not found".to_string())
    }
}

/// 手动执行任务
#[tauri::command]
pub async fn execute_task(
    task_id: u32,
    state: tauri::State<'_, TaskScheduler>,
) -> Result<String, String> {
    let (task_type, content, timeout) = {
        let tasks = state.tasks.lock().map_err(|e| e.to_string())?;
        let task = tasks
            .iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| "Task not found".to_string())?;
        (
            task.task_type.clone(),
            task.content.clone(),
            task.timeout_secs,
        )
    };

    // 创建日志
    let log_id = {
        let mut nid = state.next_log_id.lock().map_err(|e| e.to_string())?;
        let id = *nid;
        *nid += 1;
        id
    };

    let log = TaskLog {
        id: log_id,
        task_id,
        started_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        finished_at: None,
        status: "running".to_string(),
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        duration_ms: None,
    };
    state.logs.lock().map_err(|e| e.to_string())?.push(log);

    // 执行
    let result = match task_type {
        TaskType::Shell => {
            if let Some(ref c) = content {
                execute_with_sandbox("sh", &["-c", c], timeout).await
            } else {
                Err("No script content".to_string())
            }
        }
        TaskType::Python => {
            if let Some(ref c) = content {
                execute_with_sandbox("python3", &["-c", c], timeout).await
            } else {
                Err("No script content".to_string())
            }
        }
        TaskType::Shutdown => execute_shutdown().await,
    };

    // 更新日志
    {
        let mut all_logs = state.logs.lock().map_err(|e| e.to_string())?;
        if let Some(existing) = all_logs.iter_mut().find(|l| l.id == log_id) {
            let started_at = existing.started_at.clone();
            let start_time =
                chrono::NaiveDateTime::parse_from_str(&started_at, "%Y-%m-%d %H:%M:%S")
                    .unwrap_or_else(|_| Local::now().naive_local());
            let now = Local::now().naive_local();
            let duration = (now - start_time).num_milliseconds().max(0) as u64;
            existing.finished_at = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
            existing.duration_ms = Some(duration);
            match &result {
                Ok((code, stdout, stderr)) => {
                    existing.status = if *code == 0 { "success" } else { "failed" }.to_string();
                    existing.exit_code = Some(*code);
                    existing.stdout = stdout.clone();
                    existing.stderr = stderr.clone();
                }
                Err(err) => {
                    existing.status = "timeout".to_string();
                    existing.stderr = err.clone();
                }
            }
        }
    }

    // 更新任务
    {
        let mut tasks = state.tasks.lock().map_err(|e| e.to_string())?;
        if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id) {
            t.last_run = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
            t.run_count += 1;
            t.last_status = Some(match &result {
                Ok((code, _, _)) => if *code == 0 { "success" } else { "failed" }.to_string(),
                Err(_) => "timeout".to_string(),
            });
        }
    }

    // 持久化
    state.save_tasks();
    save_logs_impl(&state.logs, task_id);

    result.map(|(code, stdout, stderr)| {
        if stdout.is_empty() && stderr.is_empty() {
            format!("Exit code: {}", code)
        } else {
            format!("Exit code: {}\n{}\n{}", code, stdout, stderr)
        }
    })
}

/// 获取任务的执行日志
#[tauri::command]
pub fn get_task_logs(task_id: u32, state: tauri::State<'_, TaskScheduler>) -> Vec<TaskLog> {
    let logs = state.logs.lock().map(|l| l.clone()).unwrap_or_default();
    let mut task_logs: Vec<TaskLog> = logs.into_iter().filter(|l| l.task_id == task_id).collect();
    task_logs.sort_by_key(|b| std::cmp::Reverse(b.id));
    task_logs.truncate(50);
    task_logs
}

/// 清除任务的执行日志
#[tauri::command]
pub fn clear_task_logs(task_id: u32, state: tauri::State<'_, TaskScheduler>) -> Result<(), String> {
    {
        let mut logs = state.logs.lock().map_err(|e| e.to_string())?;
        logs.retain(|l| l.task_id != task_id);
    }
    let log_path = TaskScheduler::log_path(task_id);
    let _ = std::fs::remove_file(&log_path);
    Ok(())
}

/// 更新任务
#[tauri::command]
pub fn update_task(
    task_id: u32,
    name: Option<String>,
    cron_expression: Option<String>,
    task_type: Option<String>,
    content: Option<String>,
    timeout_secs: Option<u64>,
    state: tauri::State<'_, TaskScheduler>,
) -> Result<(), String> {
    let mut tasks = state.tasks.lock().map_err(|e| e.to_string())?;
    if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
        if let Some(n) = name {
            task.name = n;
        }
        if let Some(cron) = cron_expression {
            if let Err(e) = cron::Schedule::try_from(cron.as_str()) {
                return Err(format!("Invalid cron expression: {}", e));
            }
            task.cron_expression = cron.clone();
            task.next_run = calculate_next_run(&cron).ok().flatten();
        }
        if let Some(tt_str) = task_type {
            let new_tt = match tt_str.as_str() {
                "shell" => TaskType::Shell,
                "python" => TaskType::Python,
                "shutdown" => TaskType::Shutdown,
                _ => return Err(format!("Unknown task type: {}", tt_str)),
            };
            task.task_type = new_tt;
        }
        if let Some(c) = content.clone() {
            if c.trim().is_empty() && matches!(task.task_type, TaskType::Shell | TaskType::Python) {
                return Err("Script content cannot be empty".to_string());
            }
            task.content = Some(c);
        }
        if let Some(t) = timeout_secs {
            task.timeout_secs = t;
        }
        state.save_tasks();
        Ok(())
    } else {
        Err("Task not found".to_string())
    }
}

/// 计算下次运行时间
fn calculate_next_run(cron_expression: &str) -> Result<Option<String>, String> {
    let schedule =
        cron::Schedule::try_from(cron_expression).map_err(|e| format!("Invalid cron: {}", e))?;

    if let Some(next) = schedule.upcoming(Local).next() {
        Ok(Some(next.format("%Y-%m-%d %H:%M:%S").to_string()))
    } else {
        Ok(None)
    }
}
