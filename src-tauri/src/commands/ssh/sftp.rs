use crate::commands::ssh::connections::SshStore;
use crate::commands::ssh::session::{open, SftpHandle, SshSessionManager};
use base64::{engine::general_purpose, Engine as _};
use russh_sftp::client::SftpSession;
use serde::Serialize;

#[derive(Serialize)]
pub struct SftpEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mode: u32,
    pub mtime: i64,
}

fn meta_to_entry(name: &str, meta: &russh_sftp::protocol::FileAttributes) -> SftpEntry {
    SftpEntry {
        name: name.to_string(),
        is_dir: meta.is_dir(),
        size: meta.size.unwrap_or(0),
        mode: meta.permissions.unwrap_or(0),
        mtime: meta.mtime.unwrap_or(0) as i64,
    }
}

#[tauri::command]
pub async fn ssh_sftp_open(
    app: tauri::AppHandle,
    store: tauri::State<'_, SshStore>,
    manager: tauri::State<'_, SshSessionManager>,
    connection_id: String,
) -> Result<String, String> {
    // 复用已存在的连接会话；否则新建（与 terminal.rs 相同的取会话逻辑）
    let sid = {
        let sessions = manager.sessions.lock().await;
        sessions
            .iter()
            .find(|(_, s)| s.connection_id == connection_id)
            .map(|(sid, _)| sid.clone())
    };
    let sid = match sid {
        Some(id) => id,
        None => open(&app, &store, &manager, &connection_id).await?,
    };

    let mut sessions = manager.sessions.lock().await;
    let entry = sessions.get_mut(&sid).ok_or("NO_SESSION")?;
    let channel = entry
        .client
        .channel_open_session()
        .await
        .map_err(|e| format!("OPEN_FAILED: {e}"))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| format!("SFTP_SUBSYSTEM_FAILED: {e}"))?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| format!("SFTP_INIT_FAILED: {e}"))?;

    let sftp_id = uuid::Uuid::new_v4().to_string();
    entry
        .sftp_sessions
        .lock()
        .await
        .insert(sftp_id.clone(), SftpHandle { sftp });
    Ok(sftp_id)
}

#[tauri::command]
pub async fn ssh_sftp_list_dir(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
) -> Result<Vec<SftpEntry>, String> {
    let sessions = manager.sessions.lock().await;
    for entry in sessions.values() {
        let map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            let mut out = Vec::new();
            for f in h
                .sftp
                .read_dir(&path)
                .await
                .map_err(|e| format!("SFTP_LIST: {e}"))?
            {
                out.push(meta_to_entry(&f.file_name(), &f.metadata()));
            }
            out.sort_by(|a, b| {
                (b.is_dir as u8)
                    .cmp(&(a.is_dir as u8))
                    .then(a.name.cmp(&b.name))
            });
            return Ok(out);
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_read_file(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
    offset: u64,
    length: usize,
) -> Result<String, String> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let sessions = manager.sessions.lock().await;
    for entry in sessions.values() {
        let map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            let mut file = h
                .sftp
                .open(&path)
                .await
                .map_err(|e| format!("SFTP_OPEN: {e}"))?;
            if offset > 0 {
                file.seek(tokio::io::SeekFrom::Start(offset))
                    .await
                    .map_err(|e| format!("SFTP_SEEK: {e}"))?;
            }
            let mut buf = vec![0u8; length];
            let n = file
                .read(&mut buf)
                .await
                .map_err(|e| format!("SFTP_READ: {e}"))?;
            buf.truncate(n);
            return Ok(general_purpose::STANDARD.encode(&buf));
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_write_file(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
    data: String,
    offset: u64,
) -> Result<(), String> {
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};
    let bytes = general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("INVALID_B64: {e}"))?;
    let sessions = manager.sessions.lock().await;
    for entry in sessions.values() {
        let map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            let mut file = h
                .sftp
                .open_with_flags(
                    &path,
                    russh_sftp::protocol::OpenFlags::WRITE
                        | russh_sftp::protocol::OpenFlags::CREATE,
                )
                .await
                .map_err(|e| format!("SFTP_OPEN: {e}"))?;
            if offset > 0 {
                file.seek(tokio::io::SeekFrom::Start(offset))
                    .await
                    .map_err(|e| format!("SFTP_SEEK: {e}"))?;
            }
            file.write_all(&bytes)
                .await
                .map_err(|e| format!("SFTP_WRITE: {e}"))?;
            file.sync_all()
                .await
                .map_err(|e| format!("SFTP_SYNC: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_mkdir(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
) -> Result<(), String> {
    let sessions = manager.sessions.lock().await;
    for entry in sessions.values() {
        let map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            h.sftp
                .create_dir(&path)
                .await
                .map_err(|e| format!("SFTP_MKDIR: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_rename(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    from: String,
    to: String,
) -> Result<(), String> {
    let sessions = manager.sessions.lock().await;
    for entry in sessions.values() {
        let map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            h.sftp
                .rename(&from, &to)
                .await
                .map_err(|e| format!("SFTP_RENAME: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_delete(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
    is_dir: bool,
) -> Result<(), String> {
    let sessions = manager.sessions.lock().await;
    for entry in sessions.values() {
        let map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            if is_dir {
                h.sftp
                    .remove_dir(&path)
                    .await
                    .map_err(|e| format!("SFTP_RMDIR: {e}"))?;
            } else {
                h.sftp
                    .remove_file(&path)
                    .await
                    .map_err(|e| format!("SFTP_RM: {e}"))?;
            }
            return Ok(());
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_stat(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
) -> Result<SftpEntry, String> {
    let sessions = manager.sessions.lock().await;
    for entry in sessions.values() {
        let map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            let meta = h
                .sftp
                .metadata(&path)
                .await
                .map_err(|e| format!("SFTP_STAT: {e}"))?;
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(path.clone());
            return Ok(meta_to_entry(&name, &meta));
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}
