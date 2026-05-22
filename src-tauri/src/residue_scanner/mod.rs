pub mod fs_scanner;
pub mod known_paths;
pub mod shortcut;
pub mod snapshot;
pub use snapshot::dir_size;

#[cfg(target_os = "windows")]
pub mod registry;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod service;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 单个残留条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidueItem {
    pub path: String,
    pub size: u64,
    pub category: String,
    pub is_safe_to_delete: bool,
    pub description: String,
}

/// 一次残留扫描的完整结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResidueScan {
    pub app_name: String,
    pub directories: Vec<ResidueItem>,
    pub files: Vec<ResidueItem>,
    #[cfg(target_os = "windows")]
    pub registry_keys: Vec<ResidueItem>,
    pub services: Vec<ResidueItem>,
    pub shortcuts: Vec<ResidueItem>,
    pub total_size: u64,
    pub total_items: usize,
}

/// 跨平台扫描应用残留 — 主入口
pub fn scan_for_residues(app_name: &str, package_name: &str) -> ResidueScan {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    // 1) 已知精确路径 + 通用猜测路径
    let known_dirs: Vec<PathBuf> = known_paths::get_cleanup_paths(app_name, package_name, &home);

    // 2) 文件系统关键词扫描（在常见残留目录中递归匹配）
    let (keyword_dirs, keyword_files) = fs_scanner::scan_by_keywords(app_name, &home);

    // 3) 快捷方式扫描
    let shortcuts = shortcut::scan_shortcuts(app_name);

    // 4) 服务扫描
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let services = service::scan_services(app_name);
    #[cfg(target_os = "windows")]
    let services: Vec<ResidueItem> = Vec::new();

    // 5) Windows 注册表扫描
    #[cfg(target_os = "windows")]
    let registry_items = registry::scan_registry(app_name);
    #[cfg(target_os = "windows")]
    let registry_keys: Vec<ResidueItem> = registry_items
        .iter()
        .filter(|r| r.category == "registry")
        .cloned()
        .collect();
    #[cfg(target_os = "windows")]
    let svc_from_registry: Vec<ResidueItem> = registry_items
        .into_iter()
        .filter(|r| r.category == "service")
        .collect();

    // 合并服务结果
    #[cfg(target_os = "windows")]
    let services: Vec<ResidueItem> = svc_from_registry;

    // 6) 去重合并所有目录 (已知路径 + 关键词扫描的目录)
    let mut seen_map = std::collections::HashSet::new();
    let mut all_dirs: Vec<ResidueItem> = Vec::new();

    // 先把已知路径转为 ResidueItem
    for p in &known_dirs {
        let p_str = p.display().to_string();
        if seen_map.insert(p_str.clone()) {
            let size = if p.exists() { dir_size(p) } else { 0 };
            all_dirs.push(ResidueItem {
                path: p_str,
                size,
                category: category_for_path(p),
                is_safe_to_delete: true,
                description: String::new(),
            });
        }
    }

    // 合并 keyword_dirs
    for item in keyword_dirs {
        if seen_map.insert(item.path.clone()) {
            all_dirs.push(item);
        }
    }

    // 7) 分类：实际存在的归目录/文件，不存在的也列出（路径猜测）
    let mut directories: Vec<ResidueItem> = Vec::new();
    let mut files: Vec<ResidueItem> = Vec::new();

    for item in all_dirs {
        let p = std::path::Path::new(&item.path);
        if p.is_dir() {
            directories.push(item);
        } else if p.is_file() {
            files.push(item);
        } else {
            // 路径不存在但仍作为建议列出（size = 0）
            directories.push(item);
        }
    }

    // 独立文件（非目录）
    for item in keyword_files {
        let p = std::path::Path::new(&item.path);
        let is_dup = directories.iter().any(|d| d.path == item.path)
            || files.iter().any(|f| f.path == item.path);
        if !is_dup {
            if p.is_dir() {
                directories.push(item);
            } else {
                files.push(item);
            }
        }
    }

    // 计算总量
    let total_size: u64 = directories.iter().map(|d| d.size).sum::<u64>()
        + files.iter().map(|f| f.size).sum::<u64>()
        + services.iter().map(|s| s.size).sum::<u64>()
        + shortcuts.iter().map(|s| s.size).sum::<u64>();

    #[cfg(target_os = "windows")]
    let total_size = total_size + registry_keys.iter().map(|r| r.size).sum::<u64>();

    let total_items = directories.len()
        + files.len()
        + services.len()
        + shortcuts.len()
        + {
            #[cfg(target_os = "windows")]
            {
                registry_keys.len()
            }
            #[cfg(not(target_os = "windows"))]
            {
                0
            }
        };

    ResidueScan {
        app_name: app_name.to_string(),
        directories,
        files,
        #[cfg(target_os = "windows")]
        registry_keys,
        services,
        shortcuts,
        total_size,
        total_items,
    }
}

fn category_for_path(p: &std::path::Path) -> String {
    let s = p.display().to_string().to_lowercase();
    if s.contains("cache") || s.contains("缓存") {
        "cache".into()
    } else if s.contains("config") || s.contains("configstore") || s.contains("preference") {
        "config".into()
    } else if s.contains("log") || s.contains("logs") {
        "log".into()
    } else if s.contains("temp") || s.contains("tmp") {
        "temp".into()
    } else {
        "data".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_dir_size_nonexistent() {
        assert_eq!(dir_size(Path::new("/nonexistent_path_xyz123")), 0);
    }

    #[test]
    fn test_category_for_path_cache() {
        assert_eq!(category_for_path(Path::new("/home/user/.cache/app")), "cache");
        assert_eq!(category_for_path(Path::new("/tmp/cache/xxx")), "cache");
    }

    #[test]
    fn test_category_for_path_config() {
        assert_eq!(category_for_path(Path::new("/home/user/.config/app")), "config");
        assert_eq!(category_for_path(Path::new("/home/user/.configstore/app")), "config");
    }

    #[test]
    fn test_category_for_path_log() {
        assert_eq!(category_for_path(Path::new("/var/log/app")), "log");
        assert_eq!(category_for_path(Path::new("/home/user/logs")), "log");
    }

    #[test]
    fn test_category_for_path_temp() {
        assert_eq!(category_for_path(Path::new("/tmp/app")), "temp");
        assert_eq!(category_for_path(Path::new("/var/tmp/xxx")), "temp");
    }

    #[test]
    fn test_category_for_path_default() {
        assert_eq!(category_for_path(Path::new("/home/user/.local/share/app")), "data");
    }

    #[test]
    fn test_scan_for_residues_unknown_app() {
        let result = scan_for_residues("nonexistent_app_xyz", "nonexistent_pkg");
        // Should not crash; results will be empty on most systems
        assert_eq!(result.app_name, "nonexistent_app_xyz");
        // total_items is usize, always >= 0
    }

    #[test]
    fn test_residue_item_serialization() {
        let item = ResidueItem {
            path: "/tmp/test".into(),
            size: 1024,
            category: "cache".into(),
            is_safe_to_delete: true,
            description: "Test file".into(),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("/tmp/test"));
        assert!(json.contains("1024"));
        let deserialized: ResidueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.path, "/tmp/test");
        assert_eq!(deserialized.size, 1024);
    }

    #[test]
    fn test_residue_scan_serialization() {
        let scan = ResidueScan {
            app_name: "TestApp".into(),
            directories: vec![],
            files: vec![],
            services: vec![],
            shortcuts: vec![],
            total_size: 0,
            total_items: 0,
        };
        let json = serde_json::to_string(&scan).unwrap();
        assert!(json.contains("TestApp"));
        let deserialized: ResidueScan = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.app_name, "TestApp");
    }
}
