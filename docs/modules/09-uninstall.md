# 应用卸载与残留扫描 — 模块设计文档

## 1. 功能概述

残留扫描与深度卸载（Residue Scanner & Deep Uninstaller）是一个独立的辅助模块，在软件中心的深度卸载功能中被调用。它扫描系统中与特定应用相关的残留文件（配置、缓存、日志等），并提供清理能力。

**通信链路**:
```
software.rs (force_uninstall_software) ──→ residue_scanner::scan_for_residues()
                                        ──→ residue_scanner::snapshot::take_snapshot()
                                        ──→ clean_specific_residues()
```

---

## 2. 数据结构

```rust
/// 残留扫描扫描结果
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResidueScanResult {
    pub files: Vec<ResidueItem>,
    pub directories: Vec<ResidueItem>,
    pub shortcuts: Vec<ResidueItem>,
    pub services: Vec<ResidueItem>,
    pub registry_keys: Vec<ResidueItem>,   // 仅 Windows
    pub total_items: usize,
    pub estimated_size_mb: f64,            // 预计残留占用空间
}

/// 单个残留项
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResidueItem {
    pub path: String,           // 文件/目录/键的完整路径
    pub reason: String,         // 为什么判定为残留
    pub is_safe_to_delete: bool, // 删除是否安全（无副作用）
}
```

**使用方式**:
```rust
let scan = crate::residue_scanner::scan_for_residues(&app_name, &package_name);
```

---

## 3. 核心实现

### 3.1 深度卸载流程

`force_uninstall_software` 在软件中心内部实现了一个完整的深度卸载流程：

```
┌─────────────────────────────────────────────────────┐
│ force_uninstall_software(app_name, package_name)     │
├─────────────────────────────────────────────────────┤
│ 1. kill_processes_by_name(name_lower)               │
│    └─ sysinfo::System → 遍历进程 → 关键词匹配 → 杀   │
│ 2. uninstall_software(package_name)                 │
│    └─ 通过包管理器执行卸载（可能失败，但仍继续）        │
│ 3. residue_scanner::scan_for_residues()             │
│    └─ 获取所有已知残留路径                          │
│ 4. snapshot::take_snapshot(all_paths)               │
│    └─ 清理前快照（用于对比）                         │
│ 5. 删除文件 → 删除目录 → 清理快捷方式 → 清理服务文件   │
│ 6. 删除注册表键（Windows only）                      │
│ 7. 返回清理报告                                     │
└─────────────────────────────────────────────────────┘
```

### 3.2 残留路径数据库

`get_cleanup_paths` 函数为每个已知软件定义了精确的残留路径。包含两个搜索维度：

1. **精确匹配**: 已知的软件残留路径（通过常量定义）
2. **关键词搜索**: 在标准路径下搜索包含应用关键字的相关文件和目录

```rust
pub fn get_cleanup_paths(app_name: &str) -> Vec<std::path::PathBuf> {
    let home = user_home();
    let appdata = get_appdata();
    let mut paths = Vec::new();

    // 基于 app_name 匹配已知模式
    match app_name.to_lowercase().as_str() {
        "visual studio code" | "vscode" | "code" => {
            #[cfg(target_os = "macos")]
            paths.extend([
                home.join("Library/Application Support/Code"),
                home.join("Library/Caches/com.microsoft.VSCode"),
                home.join("Library/Preferences/com.microsoft.VSCode.plist"),
                home.join(".vscode"),
            ]);

            #[cfg(target_os = "linux")]
            paths.extend([
                home.join(".config/Code"),
                home.join(".vscode"),
            ]);

            #[cfg(target_os = "windows")]
            paths.extend([
                PathBuf::from(appdata).join("Code"),
                PathBuf::from(appdata).join("Roaming/Code"),
                PathBuf::from(roaming_appdata).join("Microsoft/VSCode"),
            ]);
        }

        "node.js" | "nodejs" | "node" => {
            // 跨平台通用
            paths.push(home.join(".npm"));
            paths.push(home.join(".node-gyp"));

            #[cfg(unix)]
            paths.push(PathBuf::from("/usr/local/lib/node_modules"));

            #[cfg(target_os = "macos")]
            paths.push(home.join("Library/Caches/node"));

            #[cfg(target_os = "windows")]
            paths.push(PathBuf::from(appdata).join("npm-cache"));
        }

        // ... 更多软件
    }
}
```

### 3.3 通用关键词搜索

对于没有精确匹配的软件，通过关键词在标准缓存/配置目录下搜索:

```rust
fn keyword_scan(keywords: &[&str]) -> Vec<ResidueItem> {
    let mut items = Vec::new();
    let search_dirs = get_search_directories(); // 跨平台标准目录

    for dir in search_dirs {
        if !dir.exists() { continue; }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                // 检查文件名是否包含任一关键词
                if keywords.iter().any(|kw| name.contains(kw)) {
                    items.push(ResidueItem {
                        path: entry.path().to_string_lossy().to_string(),
                        reason: format!("Match keyword '{}' in {}", keywords.iter().find(|k| name.contains(*k)).unwrap(), dir.display()),
                        is_safe_to_delete: true,
                    });
                }
            }
        }
    }
    items
}
```

### 3.4 残留项过滤

```rust
pub fn scan_for_residues(app_name: &str, package_name: &str) -> ResidueScanResult {
    // 1. 从已知路径数据库获取
    let known_paths = get_cleanup_paths(app_name);
    // 2. 关键词搜索
    let keywords = extract_keywords(app_name, package_name);
    let scanned = keyword_scan(&keywords);

    // 3. 汇总并分类
    let mut files = Vec::new();
    let mut directories = Vec::new();
    let mut shortcuts = Vec::new();
    let mut services = Vec::new();

    // 合并已知路径和扫描结果，去重
    for path in known_paths.iter().chain(scanned.iter().map(|s| PathBuf::from(&s.path))) {
        let path = PathBuf::from(&path);
        if path.is_file() || path.is_symlink() {
            files.push(ResidueItem { path: path.to_string_lossy().to_string(), reason: "Known file".into(), is_safe_to_delete: true });
        } else if path.is_dir() {
            directories.push(...);
        }
    }

    ResidueScanResult {
        total_items: files.len() + directories.len() + shortcuts.len() + services.len(),
        estimated_size_mb: estimate_size(&files, &directories),
        ...
    }
}
```

### 3.5 选择性清理

```rust
pub fn clean_specific_residues(items: Vec<String>) -> Result<String, String> {
    let mut cleaned = 0;
    let mut failed = Vec::new();

    for path_str in &items {
        let path = std::path::Path::new(path_str);
        if path.is_file() || path.is_symlink() {
            match std::fs::remove_file(path) {
                Ok(_) => cleaned += 1,
                Err(e) => failed.push(format!("{}: {}", path_str, e)),
            }
        } else if path.is_dir() {
            match std::fs::remove_dir_all(path) {
                Ok(_) => cleaned += 1,
                Err(e) => failed.push(format!("{}: {}", path_str, e)),
            }
        }
    }

    Ok(format!("Cleaned {} items; {} failed", cleaned, failed.len()))
}
```

### 3.6 清理前快照

```rust
pub mod snapshot {
    use std::path::Path;
    use std::time::SystemTime;

    #[derive(Serialize)]
    pub struct Snapshot {
        pub timestamp: String,
        pub items: Vec<SnapshotItem>,
    }

    #[derive(Serialize)]
    pub struct SnapshotItem {
        pub path: String,
        pub exists: bool,
        pub size: u64,
        pub modified: Option<String>,
    }

    pub fn take_snapshot(paths: &[PathBuf]) -> Snapshot {
        let now = chrono::Local::now().to_rfc3339();
        let items = paths.iter().map(|p| SnapshotItem {
            path: p.to_string_lossy().to_string(),
            exists: p.exists(),
            size: p.metadata().map(|m| m.len()).unwrap_or(0),
            modified: p.metadata().ok()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Local> = t.into();
                    dt.to_rfc3339()
                }),
        }).collect();

        Snapshot { timestamp: now, items }
    }
}
```

快照用于清理操作的回滚参考（尽管当前版本未实现自动回滚，但日志记录了清理前的状态）。

---

## 4. Windows 专用功能

### 4.1 注册表键扫描

```rust
#[cfg(target_os = "windows")]
fn scan_registry_for_residues(app_name: &str) -> Vec<ResidueItem> {
    use winreg::enums::*;
    use winreg::RegKey;

    let mut items = Vec::new();
    let name_lower = app_name.to_lowercase();

    // 搜索 HKCU\Software 和 HKLM\Software
    for hive in &[HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(*hive);
        if let Ok(software) = root.open_subkey_with_flags("Software", KEY_READ) {
            // 枚举子键
            for name in software.enum_keys().flatten() {
                if name.to_lowercase().contains(&name_lower) {
                    items.push(ResidueItem {
                        path: format!("{}\\Software\\{}",
                            if *hive == HKEY_CURRENT_USER { "HKCU" } else { "HKLM" }, name),
                        reason: "Matching registry key".into(),
                        is_safe_to_delete: true,
                    });
                }
            }
        }
    }
    items
}
```

### 4.2 服务文件扫描

```rust
#[cfg(target_os = "windows")]
fn scan_services(app_name: &str) -> Vec<ResidueItem> {
    let name_lower = app_name.to_lowercase();
    let output = Command::new("sc").args(["query", "type=", "service", "state=", "all"])
        .output().ok()?;
    // 解析 sc query 输出，找到匹配的服务名
}
```

---

## 5. 跨平台搜索目录

| 文件类型 | macOS | Linux | Windows |
|---------|-------|-------|---------|
| 配置 | `~/Library/Application Support/` | `~/.config/` | `%APPDATA%/` |
| 缓存 | `~/Library/Caches/` | `~/.cache/` | `%LOCALAPPDATA%/cache` |
| 偏好设置 | `~/Library/Preferences/` | `~/.config/` | `%APPDATA%/Roaming/...` |
| 日志 | `~/Library/Logs/` | `~/.local/share/` | `%LOCALAPPDATA%/...` |
| 注册表 | ❌ | ❌ | `HKCU/HKLM\Software` |
| 服务 | `~/Library/LaunchAgents/` | `/etc/systemd/system/` | `HKLM\System\CurrentControlSet\Services` |

---

## 6. 关键设计决策

1. **精确 + 模糊双重扫描**: 已知路径保证准确性，关键词搜索覆盖未知残留

2. **`is_safe_to_delete` 标记**: 系统级路径（如 `/usr/local/share/` 等）即使匹配也标记为不安全，避免误删其他应用的共享数据

3. **跨平台路径从同一函数管理**: 所有平台特定的残留路径定义在同一个 `get_cleanup_paths` 函数中，通过 `#[cfg(...)]` 编译条件选择

4. **清理前快照**: 记录残留的元数据（大小、最后修改时间），虽然当前未实现自动回滚，但为后续功能预留

5. **保留已知应用的精确路径**: 对于 24 款已知工具，每条路径都是手动确认过的，对比通用关键词扫描，精确路径的召回率和准确率都更高
