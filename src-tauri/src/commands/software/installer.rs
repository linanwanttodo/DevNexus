// software/installer.rs — Software Installation Logic
//
// Provides download, extraction, and installation functionality for various software packages.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Installation result information
#[derive(Serialize, Deserialize, Clone)]
pub struct InstallResult {
    pub success: bool,
    pub install_path: String,
    pub version: String,
    pub message: String,
}

/// Get platform-specific installation base directory
pub fn get_install_base_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    let base = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".local/share/devnexus/software")
    };
    #[cfg(target_os = "macos")]
    let base = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join("Library/Application Support/devnexus/software")
    };
    #[cfg(target_os = "windows")]
    let base = {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(appdata).join("devnexus/software")
    };
    std::fs::create_dir_all(&base).ok();
    base
}

/// Recursively find binary file in directory (max 5 levels deep)
pub fn find_binary_in_dir(dir: &std::path::Path, name: &str) -> Option<PathBuf> {
    let exe_name = if cfg!(target_os = "windows") {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };

    let mut dirs_to_check = vec![dir.to_path_buf()];
    let mut depth = 0;

    while !dirs_to_check.is_empty() && depth < 5 {
        let mut next_level = Vec::new();
        for current in dirs_to_check {
            if let Ok(entries) = std::fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        next_level.push(path);
                    } else if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                        if fname == exe_name || fname == name {
                            return Some(path);
                        }
                    }
                }
            }
        }
        dirs_to_check = next_level;
        depth += 1;
    }
    None
}

/// Validate version string format
pub fn is_valid_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// Extract downloaded archive and install to target directory
pub fn extract_and_install(
    filepath: &std::path::Path,
    install_dir: &std::path::Path,
    filename: &str,
    _software_name: &str,
    _default_cmd: &str,
) -> Result<(), String> {
    // Create installation directory
    std::fs::create_dir_all(install_dir)
        .map_err(|e| format!("Failed to create install dir: {}", e))?;

    // Extract based on file type
    let filename_lower = filename.to_lowercase();
    if filename_lower.ends_with(".tar.gz") || filename_lower.ends_with(".tgz") {
        let output = std::process::Command::new("tar")
            .args([
                "-xzf",
                &filepath.to_string_lossy(),
                "-C",
                &install_dir.to_string_lossy(),
            ])
            .output()
            .map_err(|e| format!("Failed to run tar: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "Extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    } else if filename_lower.ends_with(".tar.xz") {
        let output = std::process::Command::new("tar")
            .args([
                "-xJf",
                &filepath.to_string_lossy(),
                "-C",
                &install_dir.to_string_lossy(),
            ])
            .output()
            .map_err(|e| format!("Failed to run tar: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "Extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    } else if filename_lower.ends_with(".zip") {
        let output = std::process::Command::new("unzip")
            .args([
                "-o",
                &filepath.to_string_lossy(),
                "-d",
                &install_dir.to_string_lossy(),
            ])
            .output()
            .map_err(|e| format!("Failed to run unzip: {}", e))?;
        if !output.status.success() {
            // Fallback: use Rust zip library
            let file =
                std::fs::File::open(filepath).map_err(|e| format!("Failed to open zip: {}", e))?;
            let mut archive =
                zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;
            for i in 0..archive.len() {
                let mut entry = archive
                    .by_index(i)
                    .map_err(|e| format!("Failed to read zip entry: {}", e))?;
                let entry_name = entry.name().replace('\\', "/");
                let sanitized = entry_name.split('/').fold(String::new(), |acc, part| {
                    if part == ".." || part == "." || part.is_empty() {
                        // Ignore path traversal and empty parts
                        acc
                    } else if acc.is_empty() {
                        part.to_string()
                    } else {
                        format!("{}/{}", acc, part)
                    }
                });
                let outpath = install_dir.join(&sanitized);
                if !outpath.starts_with(install_dir) {
                    continue;
                }
                if entry.is_dir() {
                    std::fs::create_dir_all(&outpath).ok();
                } else {
                    if let Some(parent) = outpath.parent() {
                        std::fs::create_dir_all(parent).ok();
                    }
                    let mut outfile = std::fs::File::create(&outpath)
                        .map_err(|e| format!("Failed to create file: {}", e))?;
                    std::io::copy(&mut entry, &mut outfile)
                        .map_err(|e| format!("Failed to write file: {}", e))?;
                }
            }
        }
    } else if filename_lower.ends_with(".dmg") {
        #[cfg(target_os = "macos")]
        {
            let mount_point = install_dir.join("mount");
            std::fs::create_dir_all(&mount_point).ok();
            let output = std::process::Command::new("hdiutil")
                .args([
                    "attach",
                    &filepath.to_string_lossy(),
                    "-mountpoint",
                    &mount_point.to_string_lossy(),
                ])
                .output()
                .map_err(|e| format!("Failed to mount DMG: {}", e))?;
            if !output.status.success() {
                return Err(format!(
                    "DMG mount failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            // Copy .app bundle from mounted DMG (stdlib recursive copy;
            // 避免引入 fs_extra 依赖——它仅在这一处用到，且增加构建体积)
            if let Ok(entries) = std::fs::read_dir(&mount_point) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "app").unwrap_or(false) {
                        copy_dir_recursive(&path, install_dir)
                            .map_err(|e| format!("Failed to copy .app: {}", e))?;
                        break;
                    }
                }
            }
            // Unmount DMG
            let _ = std::process::Command::new("hdiutil")
                .args(["detach", &mount_point.to_string_lossy()])
                .output();
        }
        #[cfg(not(target_os = "macos"))]
        return Err("DMG installation is only supported on macOS".to_string());
    } else {
        // Unknown format - try to move as-is
        let dest = install_dir.join(filename);
        std::fs::copy(filepath, &dest).map_err(|e| format!("Failed to copy file: {}", e))?;
    }

    // Clean up temp file
    let _ = std::fs::remove_file(filepath);

    Ok(())
}

/// 递归复制目录到目标父目录（stdlib-only，替代 fs_extra::dir::copy）。
/// 把 `src` 整个子树拷到 `dst_parent/src.file_name()`。
/// 不跟随符号链接（macOS .app 内 Resources 等可能含符号链接，安全起见用 symlink_metadata 区分）。
/// 仅 Unix 编译（依赖 std::os::unix::fs::symlink），避免在 Windows 触发 unix-only API 错误。
#[cfg(unix)]
#[allow(dead_code)]
fn copy_dir_recursive(src: &std::path::Path, dst_parent: &std::path::Path) -> std::io::Result<()> {
    let file_name = src.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "src has no file name")
    })?;
    let dst = dst_parent.join(file_name);
    copy_dir_into(src, &dst)
}

#[cfg(unix)]
fn copy_dir_into(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_into(&from, &to)?;
        } else if file_type.is_symlink() {
            // 不跟随：保留链接目标，复制链接本身
            let target = std::fs::read_link(&from)?;
            if to.exists() {
                std::fs::remove_file(&to).ok();
            }
            std::os::unix::fs::symlink(&target, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
