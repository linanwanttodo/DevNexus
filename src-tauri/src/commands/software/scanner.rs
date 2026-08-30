// software/scanner.rs — GUI Application Scanner for Linux
//
// Scans .desktop files to build installed applications list.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Installed application information
#[derive(Serialize, Deserialize, Clone)]
pub struct InstalledApp {
    pub name: String,
    pub version: String,
    pub source: String,
    pub icon: Option<String>,
}

/// Check if a desktop file is a system/GNOME component (should be excluded)
fn is_system_desktop(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy().to_lowercase();
    let name = path
        .file_stem()
        .map(|x| x.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    s.contains("/system-applications/")
        || s.contains("/autostart/")
        || name.starts_with("gnome-")
        || name.starts_with("org.gnome.")
        || name.ends_with("-autostart")
        || name == "org.gnome.software"
        || name.contains("tracker")
        || name.contains("zeitgeist")
        || name.contains("bluetooth-sendto")
        || name == "apport-gtk"
        || name == "gnome-software"
}

/// Scan system .desktop files and build GUI app list
#[cfg(target_os = "linux")]
pub fn list_gui_apps() -> Vec<InstalledApp> {
    use std::collections::HashMap;

    let desktop_dirs = [
        dirs_home_dir().join(".local/share/applications"),
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];

    let mut apps: Vec<InstalledApp> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for (dir_idx, dir) in desktop_dirs.iter().enumerate() {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e != "desktop").unwrap_or(true) {
                continue;
            }
            if is_system_desktop(&path) {
                continue;
            }
            let file_stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if file_stem.is_empty() || file_stem.ends_with(".desktop") {
                continue;
            }
            let source = match dir_idx {
                0 => "manual",
                1 => "system",
                2 => "flatpak",
                _ => "snap",
            };
            let path_str = path.to_string_lossy().to_string();
            let actual_source = if path_str.contains("/flatpak/") {
                "flatpak"
            } else if path_str.contains("/snapd/") {
                "snap"
            } else {
                source
            };

            let name = desktop_display_name(&path);
            let dedup_key = if name.is_empty() {
                normalize_app_name(&file_stem)
            } else {
                normalize_app_name(&name)
            };
            if dedup_key.is_empty() {
                continue;
            }

            if let Some(&idx) = seen.get(&dedup_key) {
                let existing = &mut apps[idx];
                merge_versions(&mut existing.version, &desktop_version(&path));
                if existing.source == "system" && actual_source != "system" {
                    existing.source = actual_source.to_string();
                }
                if existing.icon.is_none() {
                    existing.icon = read_desktop_icon(&path);
                }
                continue;
            }

            let version = desktop_version(&path);
            let icon = read_desktop_icon(&path);
            seen.insert(dedup_key, apps.len());
            apps.push(InstalledApp {
                name,
                version,
                source: actual_source.to_string(),
                icon,
            });
        }
    }

    apps
}

/// Read display name from .desktop file (Name=, exclude localized Name[xx]=)
#[cfg(target_os = "linux")]
fn desktop_display_name(path: &std::path::Path) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        return path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
    };
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("Name=") && !line.starts_with("Name[") {
            let name = line[5..].trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Read version from .desktop file with multiple fallback strategies
#[cfg(target_os = "linux")]
fn desktop_version(path: &std::path::Path) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        return "installed".to_string();
    };
    let mut exec_line: Option<String> = None;
    let mut name_line: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line
            .strip_prefix("X-AppVersion=")
            .or_else(|| line.strip_prefix("Version="))
        {
            let v = v.trim();
            if !v.is_empty() && v != "1.0" {
                return v.to_string();
            }
        }
        if let Some(v) = line.strip_prefix("Exec=") {
            exec_line = Some(v.trim().to_string());
        }
        if let Some(v) = line.strip_prefix("Name=") {
            if !line.starts_with("Name[") {
                name_line = Some(v.trim().to_string());
            }
        }
    }
    // Extract version from Exec path
    if let Some(exec) = exec_line {
        for seg in exec.split(['/', '\\']) {
            let seg = seg.trim();
            if seg.is_empty() {
                continue;
            }
            if let Some((name, ver)) = seg.rsplit_once('-') {
                if ver.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && ver
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
                    && ver.len() <= 24
                    && !name.is_empty()
                {
                    return ver.to_string();
                }
            }
        }
    }
    // Extract version from display name
    if let Some(name) = name_line {
        let trimmed = name.trim();
        if let Some(ver) = trimmed.split_whitespace().last() {
            if ver.chars().next().is_some_and(|c| c.is_ascii_digit())
                && ver
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
                && ver.len() <= 24
            {
                return ver.to_string();
            }
        }
    }
    "installed".to_string()
}

/// Read icon from .desktop file and convert to base64 data URL
#[cfg(target_os = "linux")]
fn read_desktop_icon(path: &std::path::Path) -> Option<String> {
    desktop_icon_path(path).and_then(|p| read_image_as_data_url(&p))
}

/// Normalize app name for deduplication matching
#[cfg(target_os = "linux")]
fn normalize_app_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut base = lower
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>();
    let stripped = [
        "launcher", "desktop", "client", "player", "studio", "common", "bin",
    ];
    for suffix in stripped {
        if base.ends_with(suffix) && base.len() > suffix.len() + 2 {
            base.truncate(base.len() - suffix.len());
            break;
        }
    }
    if base.ends_with("libs") && base.len() > 6 {
        base.truncate(base.len() - 4);
    }
    base
}

/// Merge versions from multiple sources
#[cfg(target_os = "linux")]
fn merge_versions(existing: &mut String, new_version: &str) {
    let cur = existing.trim();
    let new_ver = new_version.trim();
    if cur.is_empty() {
        *existing = new_ver.to_string();
        return;
    }
    let placeholder = ["installed", "unknown", "N/A"];
    let cur_is_placeholder = placeholder.contains(&cur);
    let new_is_placeholder = placeholder.contains(&new_ver);

    if cur == new_ver || new_is_placeholder {
        return;
    }
    if cur_is_placeholder {
        *existing = new_ver.to_string();
        return;
    }
    if !cur.split(',').any(|v| v.trim() == new_ver) {
        existing.push_str(&format!(", {}", new_ver));
    }
}

/// Get home directory (Linux only)
#[cfg(target_os = "linux")]
fn dirs_home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/root"))
}

/// Read Name field from .desktop file
#[cfg(target_os = "linux")]
fn desktop_name_of(path: &std::path::Path) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        return String::new();
    };
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("Name=") && !line.starts_with("Name[") {
            return line[5..].trim().to_string();
        }
    }
    String::new()
}

/// Parse Icon= field from .desktop file and return readable path
#[cfg(target_os = "linux")]
fn desktop_icon_path(desktop: &std::path::Path) -> Option<PathBuf> {
    let content = std::fs::read_to_string(desktop).ok()?;
    let mut icon_value: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("Icon=") {
            icon_value = Some(v.trim().to_string());
            break;
        }
    }
    let icon = icon_value?;
    if icon.is_empty() {
        return None;
    }
    let icon_path = std::path::Path::new(&icon);
    if icon_path.is_absolute() && icon_path.is_file() {
        return Some(icon_path.to_path_buf());
    }
    if icon.contains('/') && !icon.starts_with('/') {
        for base in ["/usr/share", "/usr/local/share"] {
            let p = std::path::Path::new(base).join(&icon);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let name_without_ext = icon
        .rsplit_once('.')
        .map(|(n, e)| {
            if matches!(e, "png" | "svg" | "svgz" | "xpm") {
                n.to_string()
            } else {
                icon.clone()
            }
        })
        .unwrap_or_else(|| icon.clone());

    let theme_dirs = [
        "/usr/share/icons/hicolor",
        "/usr/share/icons/Adwaita",
        "/usr/share/pixmaps",
    ];
    for theme in theme_dirs {
        if let Some(icon_name_only) = name_without_ext.rsplit('/').next() {
            for size in ["512x512", "256x256", "128x128", "scalable"] {
                for ext in ["png", "svg", "svgz", "xpm"] {
                    let p = std::path::Path::new(theme)
                        .join(size)
                        .join("apps")
                        .join(format!("{}.{}", icon_name_only, ext));
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
    }
    None
}

/// Read image file and convert to base64 data URL (max 512KB)
#[cfg(target_os = "linux")]
fn read_image_as_data_url(path: &std::path::Path) -> Option<String> {
    use base64::Engine as _;
    let data = std::fs::read(path).ok()?;
    if data.is_empty() || data.len() > 512 * 1024 {
        return None;
    }
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("svg") | Some("svgz") => "image/svg+xml",
        Some("png") => "image/png",
        Some("xpm") => "image/x-xpm",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        _ => return None,
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Some(format!("data:{};base64,{}", mime, b64))
}
