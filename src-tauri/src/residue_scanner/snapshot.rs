use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 卸载前的快照记录
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct CleanupSnapshot {
    pub app_name: String,
    pub timestamp: String,
    pub directories: Vec<String>,
    pub files: Vec<SnapshotEntry>,
    #[cfg(target_os = "windows")]
    pub registry_keys: Vec<String>,
    pub total_size_before: u64,
    pub total_size_after: u64,
    pub cleaned_items: Vec<String>,
    pub failed_items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

/// 记录指定路径的快照
pub fn take_snapshot(paths: &[PathBuf]) -> Vec<SnapshotEntry> {
    let mut entries = Vec::new();
    for path in paths {
        if path.exists() {
            let meta = path.metadata();
            let size = meta.map(|m| {
                if m.is_dir() {
                    dir_size(path)
                } else {
                    m.len()
                }
            })
            .unwrap_or(0);
            entries.push(SnapshotEntry {
                path: path.display().to_string(),
                size,
                is_dir: path.is_dir(),
            });
        }
    }
    entries
}

/// 对比快照，返回已被清理的条目
#[allow(dead_code)]
pub fn diff_snapshot(before: &[SnapshotEntry]) -> Vec<String> {
    let mut cleaned = Vec::new();
    for entry in before {
        let p = std::path::Path::new(&entry.path);
        if !p.exists() {
            cleaned.push(entry.path.clone());
        }
    }
    cleaned
}

/// 计算快照中存活（仍然存在）的条目总大小
#[allow(dead_code)]
pub fn surviving_size(entries: &[SnapshotEntry]) -> u64 {
    let mut total = 0u64;
    for entry in entries {
        let p = std::path::Path::new(&entry.path);
        if p.exists() {
            total += entry.size;
        }
    }
    total
}

// 复用 residue_scanner::dir_size
pub fn dir_size(path: &std::path::Path) -> u64 {
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    if !path.is_dir() {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    total += dir_size(&entry.path());
                } else {
                    total += meta.len();
                }
            }
        }
    }
    total
}

/// 清理指定路径列表，返回 (成功列表, 失败列表)
#[allow(dead_code)]
pub fn cleanup_paths(paths: &[String]) -> (Vec<String>, Vec<String>) {
    let mut cleaned = Vec::new();
    let mut failed = Vec::new();

    for p_str in paths {
        let path = std::path::Path::new(p_str);
        if !path.exists() {
            continue;
        }
        let result = if path.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        match result {
            Ok(()) => cleaned.push(p_str.clone()),
            Err(e) => failed.push(format!("{} ({})", p_str, e)),
        }
    }

    (cleaned, failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_take_snapshot_empty() {
        let entries = take_snapshot(&[]);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_take_snapshot_nonexistent() {
        let entries = take_snapshot(&[PathBuf::from("/nonexistent_path_xyz")]);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_take_snapshot_temp_file() {
        let dir = std::env::temp_dir().join("snapshot_test_dir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.txt");
        fs::write(&file_path, b"hello").unwrap();

        let entries = take_snapshot(&[dir.clone(), file_path.clone()]);
        assert_eq!(entries.len(), 2);

        // File entry
        let file_entry = entries.iter().find(|e| e.path == file_path.display().to_string()).unwrap();
        assert_eq!(file_entry.size, 5);
        assert!(!file_entry.is_dir);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_diff_snapshot_all_gone() {
        let before = vec![
            SnapshotEntry { path: "/tmp/ghost_a".into(), size: 100, is_dir: false },
            SnapshotEntry { path: "/tmp/ghost_b".into(), size: 200, is_dir: false },
        ];
        let cleaned = diff_snapshot(&before);
        // Neither exists, both should be "cleaned"
        assert_eq!(cleaned.len(), 2);
    }

    #[test]
    fn test_surviving_size_nonexistent() {
        let entries = vec![
            SnapshotEntry { path: "/tmp/ghost_a".into(), size: 100, is_dir: false },
            SnapshotEntry { path: "/tmp/ghost_b".into(), size: 200, is_dir: false },
        ];
        // Neither exists, surviving size should be 0
        assert_eq!(surviving_size(&entries), 0);
    }

    #[test]
    fn test_cleanup_paths_nonexistent() {
        let (cleaned, failed) = cleanup_paths(&["/nonexistent_path_xyz".to_string()]);
        assert_eq!(cleaned.len(), 0);
        assert_eq!(failed.len(), 0);
    }

    #[test]
    fn test_cleanup_paths_temp_file() {
        let dir = std::env::temp_dir().join("cleanup_test_dir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("to_delete.txt");
        fs::write(&file_path, b"data").unwrap();

        let (cleaned, failed) = cleanup_paths(&[file_path.display().to_string()]);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(failed.len(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_snapshot_entry_serialization() {
        let entry = SnapshotEntry {
            path: "/tmp/test".into(),
            size: 1024,
            is_dir: false,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("/tmp/test"));
        let deserialized: SnapshotEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.size, 1024);
    }

    #[test]
    fn test_cleanup_snapshot_serialization() {
        let snap = CleanupSnapshot {
            app_name: "TestApp".into(),
            timestamp: "2024-01-01T00:00:00Z".into(),
            directories: vec!["/tmp/dir".into()],
            files: vec![],
            total_size_before: 1000,
            total_size_after: 0,
            cleaned_items: vec!["/tmp/dir".into()],
            failed_items: vec![],
        };
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("TestApp"));
        let deserialized: CleanupSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.app_name, "TestApp");
        assert_eq!(deserialized.total_size_before, 1000);
    }
}
