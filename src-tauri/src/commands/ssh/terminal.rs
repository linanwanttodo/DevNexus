use crate::commands::ssh::connections::SshStore;
use crate::commands::ssh::session::{open, SessionEntry, SshSessionManager, TerminalHandle};
use russh::ChannelMsg;
use std::sync::Arc;
use tauri::Emitter;

fn b64(data: &[u8]) -> String {
    use base64::{engine::general_purpose, Engine as _};
    general_purpose::STANDARD.encode(data)
}

#[tauri::command]
pub async fn ssh_terminal_open(
    app: tauri::AppHandle,
    store: tauri::State<'_, SshStore>,
    manager: tauri::State<'_, SshSessionManager>,
    connection_id: String,
    cols: u32,
    rows: u32,
) -> Result<String, String> {
    // 复用已存在的连接会话；否则新建
    let entry = {
        let sessions = manager.sessions.lock().await;
        sessions
            .values()
            .find(|s| s.connection_id == connection_id)
            .cloned()
    };
    let entry = match entry {
        Some(e) => e,
        None => {
            let sid = open(&app, &store, &manager, &connection_id).await?;
            manager
                .sessions
                .lock()
                .await
                .get(&sid)
                .cloned()
                .ok_or("NO_SESSION")?
        }
    };
    open_channel(&app, entry, cols, rows).await
}

async fn open_channel(
    app: &tauri::AppHandle,
    entry: Arc<SessionEntry>,
    cols: u32,
    rows: u32,
) -> Result<String, String> {
    let client = entry.client.lock().await;
    let channel = client
        .channel_open_session()
        .await
        .map_err(|e| format!("OPEN_FAILED: {e}"))?;
    channel
        .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
        .await
        .map_err(|e| format!("PTY_FAILED: {e}"))?;
    channel
        .request_shell(false)
        .await
        .map_err(|e| format!("SHELL_FAILED: {e}"))?;

    let term_id = uuid::Uuid::new_v4().to_string();
    let (read_half, write_half) = channel.split();

    // 读任务：ChannelMsg -> emit + 缓冲（AI 读屏）
    let app_clone = app.clone();
    let tid = term_id.clone();
    let entry_clone = entry.clone();
    tokio::spawn(async move {
        let mut read = read_half;
        let mut closed = false;
        while let Some(msg) = read.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    let _ = app_clone.emit(
                        "ssh-terminal-output",
                        serde_json::json!({
                            "session_id": tid, "data": b64(&data),
                        }),
                    );
                    append_buffer(&entry_clone, &tid, &data).await;
                }
                ChannelMsg::ExtendedData { data, ext: _ } => {
                    let _ = app_clone.emit(
                        "ssh-terminal-output",
                        serde_json::json!({
                            "session_id": tid, "data": b64(&data),
                        }),
                    );
                    append_buffer(&entry_clone, &tid, &data).await;
                }
                ChannelMsg::Close | ChannelMsg::Eof => {
                    closed = true;
                    break;
                }
                _ => {}
            }
        }
        let _ = app_clone.emit(
            "ssh-terminal-closed",
            serde_json::json!({
                "session_id": tid, "reason": if closed { "closed" } else { "error" },
            }),
        );
    });

    entry.terminals.lock().await.insert(
        term_id.clone(),
        Arc::new(TerminalHandle {
            write: tokio::sync::Mutex::new(write_half),
            output_buffer: tokio::sync::Mutex::new(
                crate::commands::ssh::session::TerminalBuffer::new(2000),
            ),
        }),
    );
    Ok(term_id)
}

/// 把远端输出追加到终端环形缓冲（AI 读屏上下文）。失败静默忽略。
async fn append_buffer(entry: &Arc<SessionEntry>, term_id: &str, data: &[u8]) {
    let term = entry.terminals.lock().await.get(term_id).cloned();
    if let Some(term) = term {
        let text = String::from_utf8_lossy(data);
        let mut buf = term.output_buffer.lock().await;
        buf.append(&text);
    }
}

#[tauri::command]
pub async fn ssh_terminal_input(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    use base64::{engine::general_purpose, Engine as _};
    let bytes = general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("INVALID_B64: {e}"))?;
    let term = state
        .find_terminal(&session_id)
        .await
        .ok_or_else(|| format!("NO_TERMINAL: {session_id}"))?;
    let write = term.write.lock().await;
    write
        .data_bytes(bytes)
        .await
        .map_err(|e| format!("WRITE_FAILED: {e}"))
}

#[tauri::command]
pub async fn ssh_terminal_resize(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    let term = state
        .find_terminal(&session_id)
        .await
        .ok_or_else(|| format!("NO_TERMINAL: {session_id}"))?;
    let write = term.write.lock().await;
    write
        .window_change(cols, rows, 0, 0)
        .await
        .map_err(|e| format!("RESIZE_FAILED: {e}"))
}

#[tauri::command]
pub async fn ssh_terminal_close(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
) -> Result<(), String> {
    // 先在锁内摘除句柄，EOF/close 的网络 I/O 在锁外执行
    let handle = {
        let sessions = state.sessions.lock().await;
        let mut found = None;
        for entry in sessions.values() {
            if let Some(t) = entry.terminals.lock().await.remove(&session_id) {
                found = Some(t);
                break;
            }
        }
        found
    };
    if let Some(t) = handle {
        let write = t.write.lock().await;
        let _ = write.eof().await;
        let _ = write.close().await;
    }
    Ok(())
}
