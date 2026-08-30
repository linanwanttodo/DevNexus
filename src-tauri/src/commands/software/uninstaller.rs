// software/uninstaller.rs — Software Uninstallation Logic
//
// Provides deep uninstallation and residue cleanup functionality.

use serde::{Deserialize, Serialize};

/// Uninstall result information
#[derive(Serialize, Deserialize, Clone)]
pub struct UninstallResult {
    pub success: bool,
    pub cleaned_dirs: Vec<String>,
    pub failed_dirs: Vec<String>,
    pub message: String,
}

/// Deep uninstall with source hint for package manager selection
/// Note: This is a simplified version. The full Tauri command remains in software_core.rs
pub async fn uninstall_software_deep_with_source_internal(
    _package_name: String,
    _app_name: String,
    _source: Option<String>,
) -> Result<String, String> {
    Err("Deep uninstall requires software_pm module - use software_core::uninstall_software_deep instead".to_string())
}

/// Force uninstall residues by scanning and deleting all known paths
pub fn force_uninstall_residues_blocking(app_name: &str, package_name: &str) -> Vec<String> {
    // Get all known residue paths (including keyword scanning)
    let scan = crate::residue_scanner::scan_for_residues(app_name, package_name);

    // Take snapshot for rollback (optional)
    let all_paths: Vec<std::path::PathBuf> = scan
        .directories
        .iter()
        .chain(scan.files.iter())
        .map(|i| std::path::PathBuf::from(&i.path))
        .collect();
    let _before = crate::residue_scanner::snapshot::take_snapshot(&all_paths);

    // Delete files first, then directories (recursive)
    let mut cleaned = Vec::new();
    let mut failed = Vec::new();

    // Delete files (only those marked as safe)
    for item in &scan.files {
        if !item.is_safe_to_delete {
            failed.push(format!("{} (not marked safe to delete)", item.path));
            continue;
        }
        if let Err(e) = std::fs::remove_file(&item.path) {
            failed.push(format!("{} ({})", item.path, e));
        } else {
            cleaned.push(item.path.clone());
        }
    }

    // Delete directories (only those marked as safe)
    for item in &scan.directories {
        if !item.is_safe_to_delete {
            failed.push(format!("{} (not marked safe to delete)", item.path));
            continue;
        }
        if let Err(e) = std::fs::remove_dir_all(&item.path) {
            failed.push(format!("{} ({})", item.path, e));
        } else {
            cleaned.push(item.path.clone());
        }
    }

    // Clean up shortcuts (Windows)
    #[cfg(target_os = "windows")]
    {
        for shortcut in &scan.shortcuts {
            if let Err(e) = std::fs::remove_file(&shortcut.path) {
                failed.push(format!("{} ({})", shortcut.path, e));
            } else {
                cleaned.push(shortcut.path.clone());
            }
        }
    }

    // Return summary
    let mut result = Vec::new();
    if !cleaned.is_empty() {
        result.push(format!("已清理 {} 项", cleaned.len()));
    }
    if !failed.is_empty() {
        result.push(format!("失败 {} 项: {}", failed.len(), failed.join(", ")));
    }
    if result.is_empty() {
        result.push("未发现残留".to_string());
    }
    result
}
