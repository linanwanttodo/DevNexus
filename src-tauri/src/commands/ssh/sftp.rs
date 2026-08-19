use crate::commands::ssh::connections::SshStore;
use crate::commands::ssh::session::{open, SftpHandle, SshSessionManager};
use base64::{engine::general_purpose, Engine as _};
use russh_sftp::client::SftpSession;
use serde::Serialize;
use std::sync::Arc;

/// 单次读取上限（8 MiB）：防止前端误传超大 length 撑爆内存
const MAX_READ_LEN: usize = 8 * 1024 * 1024;

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

    let entry = manager
        .sessions
        .lock()
        .await
        .get(&sid)
        .cloned()
        .ok_or("NO_SESSION")?;
    let client = entry.client.lock().await;
    let channel = client
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
    entry.sftp_sessions.lock().await.insert(
        sftp_id.clone(),
        Arc::new(SftpHandle {
            sftp: tokio::sync::Mutex::new(sftp),
        }),
    );
    Ok(sftp_id)
}

#[tauri::command]
pub async fn ssh_sftp_list_dir(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
) -> Result<Vec<SftpEntry>, String> {
    let h = manager
        .find_sftp(&sftp_id)
        .await
        .ok_or_else(|| format!("NO_SFTP: {sftp_id}"))?;
    let sftp = h.sftp.lock().await;
    let mut out = Vec::new();
    for f in sftp
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
    Ok(out)
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
    if length == 0 || length > MAX_READ_LEN {
        return Err(format!("INVALID_LENGTH: must be in 1..={}", MAX_READ_LEN));
    }
    let h = manager
        .find_sftp(&sftp_id)
        .await
        .ok_or_else(|| format!("NO_SFTP: {sftp_id}"))?;
    let sftp = h.sftp.lock().await;
    let mut file = sftp
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
    Ok(general_purpose::STANDARD.encode(&buf))
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
    let h = manager
        .find_sftp(&sftp_id)
        .await
        .ok_or_else(|| format!("NO_SFTP: {sftp_id}"))?;
    let sftp = h.sftp.lock().await;
    let mut file = sftp
        .open_with_flags(
            &path,
            russh_sftp::protocol::OpenFlags::WRITE | russh_sftp::protocol::OpenFlags::CREATE,
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
    Ok(())
}

#[tauri::command]
pub async fn ssh_sftp_mkdir(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
) -> Result<(), String> {
    let h = manager
        .find_sftp(&sftp_id)
        .await
        .ok_or_else(|| format!("NO_SFTP: {sftp_id}"))?;
    let sftp = h.sftp.lock().await;
    sftp.create_dir(&path)
        .await
        .map_err(|e| format!("SFTP_MKDIR: {e}"))
}

#[tauri::command]
pub async fn ssh_sftp_rename(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    from: String,
    to: String,
) -> Result<(), String> {
    let h = manager
        .find_sftp(&sftp_id)
        .await
        .ok_or_else(|| format!("NO_SFTP: {sftp_id}"))?;
    let sftp = h.sftp.lock().await;
    sftp.rename(&from, &to)
        .await
        .map_err(|e| format!("SFTP_RENAME: {e}"))
}

#[tauri::command]
pub async fn ssh_sftp_delete(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
    is_dir: bool,
) -> Result<(), String> {
    let h = manager
        .find_sftp(&sftp_id)
        .await
        .ok_or_else(|| format!("NO_SFTP: {sftp_id}"))?;
    let sftp = h.sftp.lock().await;
    if is_dir {
        sftp.remove_dir(&path)
            .await
            .map_err(|e| format!("SFTP_RMDIR: {e}"))?;
    } else {
        sftp.remove_file(&path)
            .await
            .map_err(|e| format!("SFTP_RM: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn ssh_sftp_stat(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
) -> Result<SftpEntry, String> {
    let h = manager
        .find_sftp(&sftp_id)
        .await
        .ok_or_else(|| format!("NO_SFTP: {sftp_id}"))?;
    let meta = h
        .sftp
        .lock()
        .await
        .metadata(&path)
        .await
        .map_err(|e| format!("SFTP_STAT: {e}"))?;
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or(path.clone());
    Ok(meta_to_entry(&name, &meta))
}
