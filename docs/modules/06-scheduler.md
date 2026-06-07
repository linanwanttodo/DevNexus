# 任务调度 — 模块设计文档

## 1. 功能概述

定时任务调度器（Task Scheduler）提供类似 cron 的定时任务功能，支持 Shell 命令执行、Python 脚本运行、系统关机/休眠/重启。任务在后台独立运行，支持日志记录和状态监控。

**通信链路**:
```
TaskScheduler.svelte ──→ invoke("add_task")         ──→ scheduler.rs
                    ──→ invoke("list_tasks")         ──→ scheduler.rs
                    ──→ invoke("delete_task")        ──→ scheduler.rs
                    ──→ invoke("toggle_task")        ──→ scheduler.rs
                    ──→ invoke("execute_task")       ──→ scheduler.rs
                    ──→ invoke("update_task")        ──→ scheduler.rs
                    ──→ invoke("get_task_logs")      ──→ scheduler.rs
                    ──→ invoke("clear_task_logs")    ──→ scheduler.rs
```

---

## 2. 数据结构

```rust
/// 任务类型
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TaskType {
    Shell,             // Shell 命令
    Python,            // Python 脚本
    SystemShutdown,    // 定时关机
    SystemSleep,       // 定时休眠/睡眠
    SystemReboot,      // 定时重启
}

/// 任务状态
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Idle,              // 空闲，等待触发
    Running,           // 正在执行
    Disabled,          // 被禁用，不会触发
}

/// 执行日志
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TaskLog {
    pub timestamp: String,     // ISO 8601 格式
    pub status: String,        // "success" / "failed" / "running"
    pub message: String,       // stdout + stderr
    pub duration_secs: u64,    // 执行耗时
}

/// 定时任务
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScheduledTask {
    pub id: u32,
    pub name: String,              // 用户自定义名称
    pub task_type: TaskType,
    pub content: Option<String>,   // Shell/Python 脚本内容
    pub cron: String,              // cron 表达式，如 "0 */2 * * *"
    pub next_run: Option<String>,  // 下一次执行时间
    pub status: TaskStatus,
    pub timeout_secs: u64,         // 超时时间（默认 300s）
    pub created_at: String,
}

/// 调度器状态（注入 Tauri State）
pub struct TaskScheduler {
    pub inner: Arc<Mutex<SchedulerInner>>,
}

struct SchedulerInner {
    tasks: Vec<ScheduledTask>,
    next_id: u32,
}
```

**前端对应** (`routes/TaskScheduler.svelte`):

```javascript
let tasks = $state([]);
let showAddModal = $state(false);
let editingTask = $state(null);

// 表单状态
let taskName = $state("");
let cronExpression = $state("");
let taskType = $state("shell");
let content = $state("");
let timeoutSecs = $state(300);
```

---

## 3. 核心实现

### 3.1 调度器状态管理

```rust
impl TaskScheduler {
    pub fn new() -> Self {
        let mut s = Self { inner: Arc::new(Mutex::new(SchedulerInner {
            tasks: Vec::new(),
            next_id: 1,
        }))};
        s.load();  // 从磁盘恢复持久化的任务
        s
    }

    // 任务持久化路径 — 跨平台
    fn tasks_path() -> std::path::PathBuf {
        // macOS:   ~/Library/Application Support/devnexus/tasks.json
        // Windows: %APPDATA%/devnexus/tasks.json
        // Linux:   ~/.local/share/devnexus/tasks.json
        utils::data_dir().join("tasks.json")
    }

    fn logs_dir() -> std::path::PathBuf {
        utils::data_dir().join("task_logs")
    }
}
```

### 3.2 后台调度循环

```rust
impl TaskScheduler {
    pub fn start_background(&self) {
        let shared = self.inner.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                {
                    let mut inner = shared.lock().unwrap();
                    let now = chrono::Local::now();
                    for task in &mut inner.tasks {
                        if task.status != TaskStatus::Idle { continue; }
                        // 检查 cron 表达式是否匹配当前时间
                        if CronExpression::new(&task.cron)
                            .and_then(|c| c.is_due(now))
                            .unwrap_or(false)
                        {
                            task.status = TaskStatus::Running;
                            task.next_run = None; // 将在执行完毕后重新计算
                        }
                    }
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });
    }
}
```

**10 秒轮询间隔**: 这是 cron 的最小精度。用户配置的 cron 表达式如 `0 */2 * * *`（每 2 小时），10 秒轮询已经远快于需要的精度。轮询间隔固定，不需要动态调整。

**cron 库**: 使用 `cron` crate（`CronExpression`）来解析和评估 cron 表达式。

### 3.3 任务执行引擎

```rust
async fn execute_task(task_id: u32) -> Result<String, String> {
    let scheduler = get_scheduler();
    let task = scheduler.get_task(task_id)?;

    let result = match task.task_type {
        TaskType::Shell => execute_shell(&task.content, task.timeout_secs).await,
        TaskType::Python => execute_python(&task.content, task.timeout_secs).await,
        TaskType::SystemShutdown => execute_system_action("shutdown").await,
        TaskType::SystemSleep => execute_system_action("sleep").await,
        TaskType::SystemReboot => execute_system_action("reboot").await,
    };

    // 记录日志
    scheduler.log_task(task_id, &result);

    // 重新计算 next_run
    scheduler.update_next_run(task_id);

    result
}
```

### 3.4 Shell/Python 脚本执行

```rust
async fn execute_shell(content: &str, timeout_secs: u64) -> Result<String, String> {
    // 写入临时脚本文件
    let script = std::env::temp_dir().join(format!("devnexus_{}.sh", uuid::Uuid::new_v4()));
    // 首行插入 shebang
    let full_content = format!("#!/bin/sh\n{}", content);
    std::fs::write(&script, full_content)
        .map_err(|e| format!("Failed to write script: {}", e))?;
    // 设置可执行权限
    #[cfg(unix)]
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .map_err(|e| format!("Failed to set permissions: {}", e))?;

    // 执行并超时控制
    let result = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        async {
            let output = tokio::process::Command::new("/bin/sh")
                .arg(&script)
                .output()
                .await
                .map_err(|e| format!("Execution failed: {}", e))?;
            // ...
        }
    ).await;

    // 清理临时文件
    let _ = std::fs::remove_file(&script);

    result.map_err(|_| "Execution timed out".to_string())?
}
```

**Python 脚本执行类似，但使用 `python3` 作为解释器**:

```rust
async fn execute_python(content: &str, timeout_secs: u64) -> Result<String, String> {
    // 写入 .py 临时文件
    let script = std::env::temp_dir().join(format!("devnexus_{}.py", uuid::Uuid::new_v4()));
    std::fs::write(&script, content)?;

    let output = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tokio::process::Command::new("python3").arg(&script).output()
    ).await;
}
```

### 3.5 系统动作执行（跨平台）

```rust
async fn system_shutdown() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    { Command::new("osascript").args(["-e", r#"tell app "System Events" to shut down"#]).output(); }
    #[cfg(target_os = "linux")]
    { Command::new("systemctl").args(["poweroff", "-i"]).output(); }
    #[cfg(target_os = "windows")]
    { Command::new("shutdown").args(["/s", "/t", "5"]).output(); }
}
```

**不同平台系统操作命令**:

| 操作 | macOS | Linux | Windows |
|------|-------|-------|---------|
| 关机 | `osascript -e 'tell app "System Events" to shut down'` | `systemctl poweroff -i` | `shutdown /s /t 5` |
| 睡眠 | `pmset sleepnow` | `systemctl suspend` | `rundll32.exe powrprof.dll,SetSuspendState 0,1,0` |
| 重启 | `osascript -e 'tell app "System Events" to restart'` | `systemctl reboot -i` | `shutdown /r /t 5` |

---

## 4. 前端实现

### 4.1 任务列表

```html
{#each tasks as task (task.id)}
  <div class="flex items-center justify-between px-4 py-3">
    <div class="flex-1">
      <div class="flex items-center gap-2">
        <span>{task.name}</span>
        <span class="task-type-badge">{task.task_type}</span>
        <span class="task-status-badge">{task.status}</span>
      </div>
      <div class="mt-1 text-sm text-nx-text-muted">
        <span>Cron: {task.cron}</span>
        {#if task.next_run}
          <span>Next: {task.next_run}</span>
        {/if}
      </div>
    </div>
    <div class="flex gap-2">
      <button onclick={() => editTask(task.id)}>Edit</button>
      <button onclick={() => executeNow(task.id)}>Run</button>
      <button onclick={() => toggleTask(task.id)}>
        {task.status === "Disabled" ? "Enable" : "Disable"}
      </button>
      <button onclick={() => showLogs(task.id)}>Logs</button>
      <button onclick={() => deleteTask(task.id)}>Delete</button>
    </div>
  </div>
{/each}
```

### 4.2 添加/编辑表单

表单支持填写: 任务名、cron 表达式、类型选择、脚本内容、超时时间。

### 4.3 日志侧边栏

点击 Logs 按钮打开侧边栏，以时间线形式显示任务执行记录：

```
[TIMESTAMP] ✓ success (2.3s)
[TIMESTAMP] ✗ failed (0.5s) — Error: Command not found
[TIMESTAMP] ◌ running...
```

---

## 5. 数据持久化

任务和日志通过 JSON 文件持久化到磁盘。

```rust
fn save(&self) -> Result<(), String> {
    let data = serde_json::to_string_pretty(&self.inner.tasks)
        .map_err(|e| e.to_string())?;
    std::fs::write(&Self::tasks_path(), data)
        .map_err(|e| e.to_string())
}

fn load(&mut self) {
    if let Ok(data) = std::fs::read_to_string(&Self::tasks_path()) {
        if let Ok(tasks) = serde_json::from_str::<Vec<ScheduledTask>>(&data) {
            self.inner.tasks = tasks;
        }
    }
}
```

**每次修改（增删改）都持久化一次**: 虽然 JSON 序列化频繁，但任务数量通常很少（< 100 个），序列化开销微秒级。

---

## 6. 测试

```rust
#[test] fn test_task_cron_match()
#[test] fn test_add_delete_task()
#[test] fn test_task_execution_timeout()
#[test] fn test_task_execution_output()
```

测试覆盖: cron 匹配逻辑、任务 CRUD、超时处理、输出记录。

---

## 7. 关键设计决策

1. **Rust 后端调度而非前端 setInterval**: 即使窗口关闭（后台运行），任务仍能触发。如果将定时逻辑放在前端 JS，一旦窗口关闭所有定时器都消失

2. **Tokio 异步执行**: 脚本执行使用 `tokio::process::Command` 而非 `std::process::Command`，避免阻塞调度器的主事件循环

3. **超时保护**: 默认 300 秒超时，防止用户输入死循环或挂起的命令阻塞系统

4. **临时文件执行**: 写入临时脚本再执行，而非通过 `sh -c` 直接执行命令，避免 shell 注入
