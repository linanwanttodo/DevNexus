use crate::residue_scanner::ResidueItem;

/// 扫描应用残留的快捷方式 / .desktop 文件 / 别名
pub fn scan_shortcuts(app_name: &str) -> Vec<ResidueItem> {
    let mut results = Vec::new();
    let name_lower = app_name.to_lowercase();
    let keywords: Vec<&str> = name_lower.split(|c: char| c.is_whitespace() || c == '-' || c == '_').collect();
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();

    // Linux: ~/.local/share/applications/*.desktop
    #[cfg(target_os = "linux")]
    {
        let desktop_dir = std::path::Path::new(&home).join(".local/share/applications");
        if desktop_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&desktop_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                        let fname = path.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                        if matches_name(&fname, &keywords) {
                            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                            results.push(ResidueItem {
                                path: path.display().to_string(),
                                size,
                                category: "shortcut".into(),
                                is_safe_to_delete: true,
                                description: "Desktop shortcut file".into(),
                            });
                        }
                    }
                }
            }
        }

        // /usr/share/applications/ 搜索（只匹配，不标注删除，这是系统级的）
        let sys_desktop = std::path::Path::new("/usr/share/applications");
        if sys_desktop.is_dir() {
            if let Ok(entries) = std::fs::read_dir(sys_desktop) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                        let fname = path.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                        if matches_name(&fname, &keywords) {
                            results.push(ResidueItem {
                                path: path.display().to_string(),
                                size: 0,
                                category: "shortcut".into(),
                                is_safe_to_delete: false, // 系统级，标记为不可自动删除
                                description: "System-level desktop entry".into(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Windows: Start Menu shortcuts
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let start_menu = std::path::Path::new(&appdata)
                .join("Microsoft/Windows/Start Menu/Programs");
            scan_lnk_recursive(&start_menu, &keywords, &mut results);
        }
        if let Ok(progdata) = std::env::var("PROGRAMDATA") {
            let all_users = std::path::Path::new(&progdata)
                .join("Microsoft/Windows/Start Menu/Programs");
            scan_lnk_recursive(&all_users, &keywords, &mut results);
        }
        // Desktop
        if let Ok(u) = std::env::var("USERPROFILE") {
            let desktop = std::path::Path::new(&u).join("Desktop");
            scan_lnk_recursive(&desktop, &keywords, &mut results);
        }
    }

    // macOS: /Applications aliases
    #[cfg(target_os = "macos")]
    {
        // 扫描 /Applications 目录中的应用名匹配
        let apps_dir = std::path::Path::new("/Applications");
        if apps_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(apps_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("app") {
                        let fname = path.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                        if matches_name(&fname, &keywords) {
                            // macOS app 是目录包，计算大小但不标记为自动删除（用户可能想保留）
                            let size = dir_size(&path);
                            results.push(ResidueItem {
                                path: path.display().to_string(),
                                size,
                                category: "shortcut".into(),
                                is_safe_to_delete: false,
                                description: "Application bundle (may still be needed)".into(),
                            });
                        }
                    }
                }
            }
        }
        // ~/Applications
        let user_apps = std::path::Path::new(&home).join("Applications");
        if user_apps.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&user_apps) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("app") {
                        let fname = path.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                        if matches_name(&fname, &keywords) {
                            results.push(ResidueItem {
                                path: path.display().to_string(),
                                size: 0,
                                category: "shortcut".into(),
                                is_safe_to_delete: true,
                                description: "User application alias".into(),
                            });
                        }
                    }
                }
            }
        }
    }

    results
}

#[cfg(target_os = "windows")]
fn scan_lnk_recursive(dir: &std::path::Path, keywords: &[&str], results: &mut Vec<ResidueItem>) {
    if !dir.is_dir() {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_lnk_recursive(&path, keywords, results);
            } else if path.extension().and_then(|e| e.to_str()) == Some("lnk") {
                let fname = path.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                if matches_name(&fname, keywords) {
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    results.push(ResidueItem {
                        path: path.display().to_string(),
                        size,
                        category: "shortcut".into(),
                        is_safe_to_delete: true,
                        description: "Start Menu / Desktop shortcut".into(),
                    });
                }
            }
        }
    }
}

fn matches_name(name: &str, keywords: &[&str]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    // 精确包含第一个关键词（通常是完整应用名）
    if keywords[0].len() >= 3 && name.contains(keywords[0]) {
        return true;
    }
    // 至少匹配 2 个非空关键词
    let match_count = keywords.iter().filter(|kw| kw.len() >= 2 && name.contains(**kw)).count();
    match_count >= 2
}
