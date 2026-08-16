use crate::commands::ssh::connections::SshStore;
use crate::commands::ssh::session::{open, SessionEntry, SshSessionManager};
use russh::ChannelMsg;
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
    let session_id = {
        let sessions = manager.sessions.lock().await;
        sessions
            .iter()
            .find(|(_, s)| s.connection_id == connection_id)
            .map(|(sid, _)| sid.clone())
    };
    let sid = match session_id {
        Some(id) => {
            // 该连接已有会话，取它的 client 直接开通道
            let mut sessions = manager.sessions.lock().await;
            let entry = sessions.get_mut(&id).ok_or("NO_SESSION")?;
            open_channel(&app, entry, cols, rows).await?
        }
        None => {
            let sid = open(&app, &store, &manager, &connection_id).await?;
            let mut sessions = manager.sessions.lock().await;
            let entry = sessions.get_mut(&sid).ok_or("NO_SESSION")?;
            open_channel(&app, entry, cols, rows).await?
        }
    };
    Ok(sid)
}

async fn open_channel(
    app: &tauri::AppHandle,
    entry: &mut SessionEntry,
    cols: u32,
    rows: u32,
) -> Result<String, String> {
    let channel = entry
        .client
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

    // 读任务：ChannelMsg -> emit
    let app_clone = app.clone();
    let tid = term_id.clone();
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
                }
                ChannelMsg::ExtendedData { data, ext: _ } => {
                    let _ = app_clone.emit(
                        "ssh-terminal-output",
                        serde_json::json!({
                            "session_id": tid, "data": b64(&data),
                        }),
                    );
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
        crate::commands::ssh::session::TerminalHandle { write: write_half },
    );
    Ok(term_id)
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
    let sessions = state.sessions.lock().await;
    for entry in sessions.values() {
        let terms = entry.terminals.lock().await;
        if let Some(t) = terms.get(&session_id) {
            t.write
                .data_bytes(bytes)
                .await
                .map_err(|e| format!("WRITE_FAILED: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("NO_TERMINAL: {session_id}"))
}

#[tauri::command]
pub async fn ssh_terminal_resize(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    let sessions = state.sessions.lock().await;
    for entry in sessions.values() {
        let terms = entry.terminals.lock().await;
        if let Some(t) = terms.get(&session_id) {
            t.write
                .window_change(cols, rows, 0, 0)
                .await
                .map_err(|e| format!("RESIZE_FAILED: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("NO_TERMINAL: {session_id}"))
}

#[tauri::command]
pub async fn ssh_terminal_close(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
) -> Result<(), String> {
    let sessions = state.sessions.lock().await;
    for entry in sessions.values() {
        let mut terms = entry.terminals.lock().await;
        if let Some(t) = terms.remove(&session_id) {
            let _ = t.write.eof().await;
            let _ = t.write.close().await;
            return Ok(());
        }
    }
    Ok(())
}
