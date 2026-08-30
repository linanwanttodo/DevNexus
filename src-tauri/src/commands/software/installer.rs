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
            // Copy .app bundle from mounted DMG
            if let Ok(entries) = std::fs::read_dir(&mount_point) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "app").unwrap_or(false) {
                        let dest =
                            install_dir.join(path.file_name().expect("App bundle has no name"));
                        fs_extra::dir::copy(&path, install_dir, &fs_extra::dir::CopyOptions::new())
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
