use crate::residue_scanner::ResidueItem;

/// 扫描 Linux systemd 服务 / macOS launchd plist 中匹配应用名的残留
pub fn scan_services(app_name: &str) -> Vec<ResidueItem> {
    let mut results = Vec::new();
    let name_lower = app_name.to_lowercase();
    let home = std::env::var("HOME").unwrap_or_default();

    // Linux: systemd user services
    #[cfg(target_os = "linux")]
    {
        let paths = [
            std::path::Path::new(&home).join(".config/systemd/user"),
            std::path::Path::new("/etc/systemd/system").to_path_buf(),
            std::path::Path::new("/usr/lib/systemd/system").to_path_buf(),
        ];
        for dir in &paths {
            if !dir.is_dir() {
                continue;
            }
            let is_system = dir.starts_with("/etc") || dir.starts_with("/usr");
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let is_service = path.extension().and_then(|e| e.to_str()) == Some("service");
                    let is_timer = path.extension().and_then(|e| e.to_str()) == Some("timer");
                    if is_service || is_timer {
                        let fname = path.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                        if fname.contains(&name_lower) || name_lower.contains(&fname) {
                            results.push(ResidueItem {
                                path: path.display().to_string(),
                                size: 0,
                                category: "service".into(),
                                is_safe_to_delete: !is_system,
                                description: if is_service { "systemd service file".into() } else { "systemd timer file".into() },
                            });
                        }
                    }
                }
            }
        }
    }

    // macOS: LaunchAgents & LaunchDaemons
    #[cfg(target_os = "macos")]
    {
        let search_dirs = [
            (format!("{}/Library/LaunchAgents", home), true),
            ("/Library/LaunchAgents".into(), false),
            ("/Library/LaunchDaemons".into(), false),
            ("/System/Library/LaunchDaemons".into(), false),
        ];

        for (dir_str, is_user) in &search_dirs {
            let dir = std::path::Path::new(dir_str);
            if !dir.is_dir() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let ext = path.extension().and_then(|e| e.to_str());
                    if ext != Some("plist") {
                        continue;
                    }
                    let fname = path.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                    if fname.contains(&name_lower) || name_lower.contains(&fname) {
                        let is_system = !is_user;
                        results.push(ResidueItem {
                            path: path.display().to_string(),
                            size: 0,
                            category: "service".into(),
                            is_safe_to_delete: !is_system,
                            description: if *is_user { "User launchd agent".into() } else { "System launchd daemon".into() },
                        });
                    }
                }
            }
        }
    }

    results
}
