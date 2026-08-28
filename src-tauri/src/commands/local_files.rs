// src-tauri/src/commands/local_files.rs
//! 受控的本地文本文件读写命令，替代前端对 plugin-fs 的直接调用。
//!
//! 背景：此前 capabilities 中 `fs:allow-write-file` / `fs:allow-read-text-file`
//! 的 scope 为 `"**"`（全盘），渲染进程一旦被攻破即可读写任意文件。
//! 改为后端命令后：路径必须通过 `validate_abs_sane_path`，
//! 且叠加大小上限，攻击面显著收窄。

use crate::utils::path_guard;

/// 单次读取上限：迁移清单/SSH 配置等文本远小于此值
pub const MAX_READ_BYTES: u64 = 8 * 1024 * 1024;
/// 单次写入上限
pub const MAX_WRITE_BYTES: usize = 16 * 1024 * 1024;

/// 读取本地文本文件（带路径校验与大小上限）
#[tauri::command]
pub fn local_read_text(path: String) -> Result<String, String> {
    let p = path_guard::validate_abs_sane_path(&path)?;
    let meta = std::fs::metadata(&p).map_err(|e| format!("READ_STAT {}: {e}", p.display()))?;
    if meta.is_dir() {
        return Err(format!("Is a directory: {}", p.display()));
    }
    if meta.len() > MAX_READ_BYTES {
        return Err(format!(
            "File too large ({} > {MAX_READ_BYTES} bytes): {}",
            meta.len(),
            p.display()
        ));
    }
    std::fs::read_to_string(&p).map_err(|e| format!("Failed to read {}: {e}", p.display()))
}

/// 写入本地文本文件（带路径校验与大小上限）
#[tauri::command]
pub fn local_write_text(path: String, content: String) -> Result<String, String> {
    let p = path_guard::validate_abs_sane_path(&path)?;
    if content.len() > MAX_WRITE_BYTES {
        return Err(format!(
            "Content too large ({} > {MAX_WRITE_BYTES} bytes)",
            content.len()
        ));
    }
    // 父目录必须已存在（导出目标来自系统文件对话框，目录总是存在的），
    // 不隐式创建目录，避免被当作任意位置写文件的跳板
    let parent_ok = p.parent().map(|d| d.is_dir()).unwrap_or(false);
    if !parent_ok {
        return Err(format!("Target directory does not exist: {}", p.display()));
    }
    std::fs::write(&p, content).map_err(|e| format!("Failed to write {}: {e}", p.display()))?;
    Ok(p.display().to_string())
}

/// 创建本地目录（含父目录）。用于 SFTP 目录递归下载时在用户选择的目标目录下
/// 还原远端目录结构（目标根目录来自系统目录对话框，子目录由前端按远端结构拼接）。
#[tauri::command]
pub fn local_mkdir_all(path: String) -> Result<String, String> {
    let p = path_guard::validate_abs_sane_path(&path)?;
    std::fs::create_dir_all(&p).map_err(|e| format!("Failed to create {}: {e}", p.display()))?;
    Ok(p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_rejects_when_parent_missing() {
        let dir = std::env::temp_dir().join(format!("dnx_local_files_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let target = dir.join("sub/out.txt");
        let res = local_write_text(target.to_string_lossy().to_string(), "x".into());
        assert!(res.is_err(), "must not implicitly create directories");
    }

    #[test]
    fn test_read_rejects_directory() {
        let res = local_read_text(std::env::temp_dir().to_string_lossy().to_string());
        assert!(res.is_err());
    }

    #[test]
    fn test_read_rejects_bad_path() {
        assert!(local_read_text("../relative.txt".into()).is_err());
    }

    #[test]
    fn test_write_roundtrip() {
        let dir = std::env::temp_dir().join(format!("dnx_local_files_ok_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("out.json");
        let res = local_write_text(target.to_string_lossy().to_string(), r#"{"a":1}"#.into());
        assert!(res.is_ok(), "err: {res:?}");
        let back = local_read_text(target.to_string_lossy().to_string()).unwrap();
        assert_eq!(back, r#"{"a":1}"#);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
