use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

// 终端会话结构
#[allow(dead_code)]
pub struct TerminalSession {
    pub id: u32,
    pub writer: std::sync::Arc<std::sync::Mutex<Box<dyn std::io::Write + Send>>>,
    pub reader: Box<dyn std::io::Read + Send>,
    pub master_handle: Option<std::sync::Arc<std::sync::Mutex<Box<dyn portable_pty::MasterPty + Send>>>>,
}

// 全局终端会话存储
pub struct TerminalState {
    pub sessions: Arc<Mutex<std::collections::HashMap<u32, TerminalSession>>>,
    pub next_id: Arc<Mutex<u32>>,
}

impl TerminalState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(std::collections::HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }
}

/// 跨平台检测最佳 shell
fn detect_shell() -> String {
    #[cfg(windows)]
    {
        // 优先级: pwsh > powershell > cmd
        for shell in &["pwsh", "powershell", "cmd"] {
            if which::which(shell).is_ok() {
                return shell.to_string();
            }
        }
        "cmd.exe".to_string()
    }

    #[cfg(unix)]
    {
        if let Ok(shell) = std::env::var("SHELL") {
            if !shell.is_empty() && std::path::Path::new(&shell).exists() {
                return shell;
            }
        }
        // fallback: 尝试常见 shell
        for shell in &["/bin/bash", "/bin/zsh", "/bin/sh"] {
            if std::path::Path::new(shell).exists() {
                return shell.to_string();
            }
        }
        "/bin/sh".to_string()
    }
}

/// 生成新的终端会话
#[tauri::command]
pub async fn spawn_terminal(app: AppHandle, state: State<'_, TerminalState>) -> Result<u32, String> {
    let pty_system = native_pty_system();
    
    // 创建 PTY pair
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to create PTY: {}", e))?;
    
    // 获取当前用户的 shell（跨平台）
    let shell = detect_shell();
    
    // 构建命令
    let mut cmd = CommandBuilder::new(shell);
    cmd.env("TERM", "xterm-256color");
    
    // 在 PTY 中启动 shell
    let _child = pair.slave.spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;
    
    // 获取读写器和 master
    let reader_for_thread = pair.master.try_clone_reader()
        .map_err(|e| format!("Failed to clone reader for thread: {}", e))?;
    let reader = pair.master.try_clone_reader()
        .map_err(|e| format!("Failed to clone reader: {}", e))?;
    let writer = pair.master.take_writer()
        .map_err(|e| format!("Failed to take writer: {}", e))?;
    let writer_arc = std::sync::Arc::new(std::sync::Mutex::new(writer));
    let master_handle = std::sync::Arc::new(std::sync::Mutex::new(pair.master));
    
    // 分配会话 ID
    let mut next_id = state.next_id.lock().map_err(|e| e.to_string())?;
    let session_id = *next_id;
    *next_id += 1;
    drop(next_id);
    
    // 存储会话
    let session = TerminalSession {
        id: session_id,
        writer: writer_arc,
        reader,
        master_handle: Some(master_handle),
    };
    
    state.sessions.lock()
        .map_err(|e| e.to_string())?
        .insert(session_id, session);
    
    // 启动后台任务读取 PTY 输出并发送到前端
    let sessions = state.sessions.clone();
    let app_handle = app.clone();
    
    tokio::task::spawn_blocking(move || {
        let mut buffer = [0u8; 1024];
        let mut reader = reader_for_thread;
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let output = String::from_utf8_lossy(&buffer[..n]).to_string();
                    let _ = app_handle.emit("terminal-output", serde_json::json!({
                        "session_id": session_id,
                        "data": output
                    }));
                }
                Err(_) => break,
            }
        }
        
        let _ = sessions.lock().unwrap().remove(&session_id);
        let _ = app_handle.emit("terminal-closed", serde_json::json!({
            "session_id": session_id
        }));
    });
    
    Ok(session_id)
}

/// 向终端写入数据
#[tauri::command]
pub async fn write_to_terminal(session_id: u32, data: String, state: State<'_, TerminalState>) -> Result<(), String> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    
    if let Some(session) = sessions.get(&session_id) {
        use std::io::Write;
        let mut writer = session.writer.lock().map_err(|e| e.to_string())?;
        writer.write_all(data.as_bytes())
            .map_err(|e| format!("Failed to write: {}", e))?;
        Ok(())
    } else {
        Err(format!("Terminal session {} not found", session_id))
    }
}

/// 关闭终端会话
#[tauri::command]
pub async fn close_terminal(session_id: u32, state: State<'_, TerminalState>) -> Result<(), String> {
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    
    if sessions.remove(&session_id).is_some() {
        Ok(())
    } else {
        Err(format!("Terminal session {} not found", session_id))
    }
}

/// 调整终端大小
#[tauri::command]
pub async fn resize_terminal(session_id: u32, cols: u16, rows: u16, state: State<'_, TerminalState>) -> Result<(), String> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    
    if let Some(session) = sessions.get(&session_id) {
        if let Some(master_arc) = &session.master_handle {
            let master = master_arc.lock().map_err(|e| e.to_string())?;
            
            let size = portable_pty::PtySize {
                rows: rows as u16,
                cols: cols as u16,
                pixel_width: 0,
                pixel_height: 0,
            };
            
            master.resize(size).map_err(|e| format!("Resize failed: {}", e))?;
            Ok(())
        } else {
            Err("No master handle available".to_string())
        }
    } else {
        Err(format!("Terminal session {} not found", session_id))
    }
}
