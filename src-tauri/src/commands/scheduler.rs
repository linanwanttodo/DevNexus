use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use chrono::Local;
use std::process::Command;
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone)]
pub struct ScheduledTask {
    pub id: u32,
    pub name: String,
    pub cron_expression: String,
    pub task_type: String, // "script", "shutdown", "email", "command"
    pub script_content: Option<String>,
    pub command: Option<String>,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub run_count: u32,
}

pub struct TaskScheduler {
    pub tasks: Arc<Mutex<Vec<ScheduledTask>>>,
    pub next_id: Arc<Mutex<u32>>,
}

impl TaskScheduler {
    pub fn new() -> Self {
        let mut s = Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        };
        s.load();
        s
    }

    fn data_path() -> std::path::PathBuf {
        let base = if cfg!(target_os = "macos") {
            std::env::var("HOME")
                .map(|h| std::path::PathBuf::from(h).join("Library/Application Support"))
                .unwrap_or_default()
        } else if cfg!(target_os = "windows") {
            std::env::var("APPDATA")
                .map(std::path::PathBuf::from)
                .unwrap_or_default()
        } else {
            std::env::var("HOME")
                .map(|h| std::path::PathBuf::from(h).join(".config"))
                .unwrap_or_default()
        };
        base.join("devnexus").join("tasks.json")
    }

    fn save(&self) {
        let path = Self::data_path();
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
        let path = Self::data_path();
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
    }

    /// 后台自动调度循环：每 30 秒检查一次所有已启用任务的 cron 表达式，
    /// 自动执行到期的任务。在 app setup 阶段调用。
    pub fn start_background(&self) {
        const CHECK_INTERVAL_SECS: u64 = 30;
        let tasks_arc = self.tasks.clone();

        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(CHECK_INTERVAL_SECS));
            interval.tick().await; // 跳过立即执行

            loop {
                interval.tick().await;
                let now = Local::now();
                let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
                let mut due: Vec<ScheduledTask> = Vec::new();

                // 收集需要执行的任务（尽量缩短持有锁的时间）
                {
                    let Ok(tasks) = tasks_arc.lock() else { continue };
                    for t in tasks.iter() {
                        if !t.enabled { continue; }
                        let should_run = match &t.next_run {
                            Some(nr) => nr.as_str() <= now_str.as_str(),
                            None => true, // 从未计算过 next_run，尝试计算
                        };
                        if should_run {
                            due.push(t.clone());
                        }
                    }
                }

                for task in &due {
                    let result = match task.task_type.as_str() {
                        "shutdown" => execute_shutdown().await,
                        "email" => {
                            Ok("Email sending simulated (needs SMTP configuration)".to_string())
                        }
                        "script" => {
                            if let Some(script) = &task.script_content {
                                execute_script(script).await
                            } else {
                                Err("No script content".to_string())
                            }
                        }
                        "command" => {
                            if let Some(cmd) = &task.command {
                                execute_command(cmd).await
                            } else {
                                Err("No command specified".to_string())
                            }
                        }
                        _ => Err(format!("Unknown task type: {}", task.task_type)),
                    };

                    // 更新任务状态
                    {
                        let Ok(mut tasks) = tasks_arc.lock() else { continue };
                        if let Some(t) = tasks.iter_mut().find(|t| t.id == task.id) {
                            t.last_run = Some(
                                Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                            );
                            t.run_count += 1;
                            if result.is_ok() {
                                let cron_str = &t.cron_expression.clone();
                                t.next_run = calculate_next_run(cron_str).ok().flatten();
                            }
                        }
                    }

                    Self::save_to_disk(&tasks_arc);
                }
            }
        });
    }

    fn save_to_disk(tasks_arc: &Arc<Mutex<Vec<ScheduledTask>>>) {
        let path = Self::data_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(tasks) = tasks_arc.lock() {
            if let Ok(json) = serde_json::to_string_pretty(&*tasks) {
                let _ = std::fs::write(&path, json);
            }
        }
    }
}

/// 添加定时任务
#[tauri::command]
pub fn add_task(
    name: String,
    cron_expression: String,
    task_type: String,
    script_content: Option<String>,
    command: Option<String>,
    state: tauri::State<'_, TaskScheduler>,
) -> Result<u32, String> {
    // 验证 cron 表达式
    if let Err(e) = cron::Schedule::try_from(cron_expression.as_str()) {
        return Err(format!("Invalid cron expression: {}", e));
    }

    let mut next_id = state.next_id.lock().map_err(|e| e.to_string())?;
    let id = *next_id;
    *next_id += 1;
    drop(next_id);

    let task = ScheduledTask {
        id,
        name,
        cron_expression: cron_expression.clone(),
        task_type,
        script_content,
        command,
        enabled: true,
        last_run: None,
        next_run: calculate_next_run(&cron_expression)?,
        run_count: 0,
    };

    state.tasks.lock()
        .map_err(|e| e.to_string())?
        .push(task);

    state.save();

    Ok(id)
}

/// 获取所有任务
#[tauri::command]
pub fn list_tasks(state: tauri::State<'_, TaskScheduler>) -> Vec<ScheduledTask> {
    state.tasks.lock()
        .map(|tasks| tasks.clone())
        .unwrap_or_else(|_| Vec::new())
}

/// 删除任务
#[tauri::command]
pub fn delete_task(task_id: u32, state: tauri::State<'_, TaskScheduler>) -> Result<(), String> {
    let mut tasks = state.tasks.lock().map_err(|e| e.to_string())?;
    tasks.retain(|t| t.id != task_id);
    drop(tasks);
    state.save();
    Ok(())
}

/// 启用/禁用任务
#[tauri::command]
pub fn toggle_task(task_id: u32, state: tauri::State<'_, TaskScheduler>) -> Result<(), String> {
    let mut tasks = state.tasks.lock().map_err(|e| e.to_string())?;
    if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
        task.enabled = !task.enabled;
        drop(tasks);
        state.save();
        Ok(())
    } else {
        Err("Task not found".to_string())
    }
}

/// 手动执行任务
#[tauri::command]
pub async fn execute_task(task_id: u32, state: tauri::State<'_, TaskScheduler>) -> Result<String, String> {
    let task = {
        let tasks = state.tasks.lock().map_err(|e| e.to_string())?;
        tasks.iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| "Task not found".to_string())?
            .clone()
    };

    let result = match task.task_type.as_str() {
        "shutdown" => execute_shutdown().await,
        "email" => Ok("Email sending simulated (needs SMTP configuration)".to_string()),
        "script" => {
            if let Some(script) = &task.script_content {
                execute_script(script).await
            } else {
                Err("No script content".to_string())
            }
        }
        "command" => {
            if let Some(cmd) = &task.command {
                execute_command(cmd).await
            } else {
                Err("No command specified".to_string())
            }
        }
        _ => Err(format!("Unknown task type: {}", task.task_type)),
    };

    {
        let mut tasks = state.tasks.lock().map_err(|e| e.to_string())?;
        if let Some(t) = tasks.iter_mut().find(|t| t.id == task_id) {
            t.last_run = Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
            t.run_count += 1;
            if result.is_ok() {
                t.next_run = calculate_next_run(&t.cron_expression)?;
            }
        }
        drop(tasks);
        state.save();
    }

    result
}

/// 计算下次运行时间
fn calculate_next_run(cron_expression: &str) -> Result<Option<String>, String> {
    let schedule = cron::Schedule::try_from(cron_expression)
        .map_err(|e| format!("Invalid cron: {}", e))?;
    
    if let Some(next) = schedule.upcoming(Local).next() {
        Ok(Some(next.format("%Y-%m-%d %H:%M:%S").to_string()))
    } else {
        Ok(None)
    }
}

/// 执行关机命令
async fn execute_shutdown() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .args(["-e", "tell application \"Finder\" to shut down"])
            .output()
            .map_err(|e| format!("Failed to execute shutdown: {}", e))?;
        if output.status.success() {
            Ok("System shutdown initiated".to_string())
        } else {
            Err(format!("Shutdown failed: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        let output = Command::new("systemctl")
            .args(["poweroff"])
            .output()
            .map_err(|e| format!("Failed to execute systemctl: {}", e))?;
        if output.status.success() {
            Ok("System shutdown initiated".to_string())
        } else {
            Err(format!("Shutdown failed (may require polkit permission): {}", String::from_utf8_lossy(&output.stderr)))
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("shutdown")
            .args(["/s", "/t", "0"])
            .output()
            .map_err(|e| format!("Failed to execute shutdown: {}", e))?;
        if output.status.success() {
            Ok("System shutdown initiated".to_string())
        } else {
            Err(format!("Shutdown failed: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }
}

/// 执行脚本
async fn execute_script(script: &str) -> Result<String, String> {
    #[cfg(unix)]
    {
        let output = Command::new("bash")
            .arg("-c")
            .arg(script)
            .output()
            .map_err(|e| e.to_string())?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
    
    #[cfg(windows)]
    {
        let output = Command::new("powershell")
            .arg("-Command")
            .arg(script)
            .output()
            .map_err(|e| e.to_string())?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

/// 执行命令
async fn execute_command(cmd: &str) -> Result<String, String> {
    #[cfg(unix)]
    {
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .map_err(|e| e.to_string())?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
    
    #[cfg(windows)]
    {
        let output = Command::new("cmd")
            .arg("/C")
            .arg(cmd)
            .output()
            .map_err(|e| e.to_string())?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
