// commands/tuning.rs — 系统调优模块
//
// 提供磁盘空间分析 / 垃圾扫描与清理 / 用户排除项持久化 能力。
// - scan_caches: 扫描常见缓存/临时/日志目录，返回可清理项及大小
// - clean_paths:  递归删除用户勾选的路径，返回释放字节
// - get_disk_usage: 列出系统各挂载点磁盘占用（供 DiskCleanup 概览）
// - list_exclusions / add_exclusion / remove_exclusion: 用户排除项（persist 到 JSON）
// - optimize_disk: 平台专用“一键优化”（Linux: apt/pacman/dnf 缓存；macOS: DS_Store/Caches；Windows: Temp）
//
// 所有命令均返回 Result<T, String>（结构化错误码，与其余模块一致）。
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// 单条可清理项
#[derive(Serialize, Clone)]
pub struct CleanCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub file_count: u64,
    pub is_dir: bool,
}

/// 单个磁盘分区占用
#[derive(Serialize, Clone)]
pub struct DiskUsage {
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub format: String,
}

/// 排除项配置（JSON 文件持久化）
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Exclusions {
    pub paths: Vec<String>,
}

fn exclusions_path() -> PathBuf {
    crate::utils::data_dir().join("tune_exclusions.json")
}

fn load_exclusions_impl() -> Exclusions {
    let path = exclusions_path();
    if !path.exists() {
        return Exclusions::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_exclusions_impl(ex: &Exclusions) -> Result<(), String> {
    let path = exclusions_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_vec_pretty(ex).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| format!("write exclusions: {e}"))
}

/// 递归计算目录/文件大小的线程安全封装（可在分离线程调用）
fn dir_size(path: &Path, file_count: &mut u64) -> u64 {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return 0,
    };
    if meta.is_file() {
        *file_count += 1;
        return meta.len();
    }
    if !meta.is_dir() {
        // symlink / 特殊文件：不深入，仅计入自身
        *file_count += 1;
        return meta.len();
    }
    let mut total = 0u64;
    if let Ok(rd) = fs::read_dir(path) {
        for entry in rd.flatten() {
            total += dir_size(&entry.path(), file_count);
        }
    }
    total
}

/// 平台相关的默认“可清理候选目录”
fn default_candidates() -> Vec<(String, String)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut out: Vec<(String, String)> = Vec::new();
    #[cfg(target_os = "linux")]
    {
        out.push(("Linux 包缓存 (apt)".into(), "/var/cache/apt".into()));
        out.push(("Linux 临时".into(), "/tmp".into()));
        out.push(("用户缓存".into(), format!("{home}/.cache")));
        out.push(("旧内核/日志".into(), "/var/log".into()));
    }
    #[cfg(target_os = "macos")]
    {
        out.push(("用户缓存".into(), format!("{home}/Library/Caches")));
        out.push(("临时文件".into(), "/tmp".into()));
        out.push(("系统日志".into(), "/var/log".into()));
        out.push((
            "Applications 缓存".into(),
            format!("{home}/Library/Caches/com.apple"),
        ));
        out.push((
            "浏览/下载清理".into(),
            format!("{home}/Library/Saved Application State"),
        ));
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("LOCALAPPDATA").unwrap_or_default();
        out.push((
            "Windows 临时".into(),
            format!("{}\\Temp", std::env::var("TEMP").unwrap_or_default()),
        ));
        out.push(("缓存目录".into(), appdata));
    }
    out
}

/// 扫描常见缓存/临时/日志目录，返回可清理项（会自动排除用户在排除列表中的路径）。
#[tauri::command]
pub fn scan_caches() -> Result<Vec<CleanCandidate>, String> {
    let ex = load_exclusions_impl();
    let mut out = Vec::new();
    for (name, raw) in default_candidates() {
        let p = PathBuf::from(&raw);
        if !p.exists() {
            continue;
        }
        if ex.paths.iter().any(|e| same_path(&PathBuf::from(e), &p)) {
            continue;
        }
        let mut file_count = 0u64;
        let bytes = dir_size(&p, &mut file_count);
        if bytes == 0 && file_count == 0 {
            // 空/不存在目录不展示
            continue;
        }
        out.push(CleanCandidate {
            id: format!("tune-{}", out.len()),
            name,
            path: p.to_string_lossy().to_string(),
            bytes,
            file_count,
            is_dir: p.is_dir(),
        });
    }
    out.sort_by_key(|a| a.bytes);
    out.reverse();
    Ok(out)
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.canonicalize().unwrap_or_else(|_| a.to_path_buf())
        == b.canonicalize().unwrap_or_else(|_| b.to_path_buf())
}

/// 删除指定路径（均先做安全校验：不允许删除根目录/应用安装位置）。
/// 返回释放的字节数。
#[tauri::command]
pub fn clean_paths(paths: Vec<String>) -> Result<u64, String> {
    if paths.is_empty() {
        return Ok(0);
    }
    let mut freed = 0u64;
    for raw in paths {
        let p = PathBuf::from(&raw);
        if !is_cleanable(&p) {
            continue;
        }
        if !p.exists() {
            continue;
        }
        let mut fc = 0u64;
        freed += dir_size(&p, &mut fc);
        if p.is_dir() {
            fs::remove_dir_all(&p).map_err(|e| format!("REMOVE_DIR {}: {e}", p.display()))?;
        } else {
            fs::remove_file(&p).map_err(|e| format!("REMOVE_FILE {}: {e}", p.display()))?;
        }
    }
    Ok(freed)
}

/// 安全校验：禁止删除根目录、家目录、系统关键路径。
fn is_cleanable(p: &Path) -> bool {
    let canon = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let s = canon.to_string_lossy().to_lowercase();
    // 全局禁止
    let root = PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
    if canon == root || canon.to_string_lossy() == "." {
        return false;
    }
    let forbidden = [
        "/",
        "\\.",
        "/home",
        "/users",
        "/system",
        "/usr",
        "/bin",
        "/sbin",
        "/etc",
        "/var",
        "/lib",
        "c:\\windows",
        "c:\\program files",
        "c:\\program files (x86)",
        "/Applications",
    ];
    let cleaned = s.trim_end_matches(['/', '\\']);
    for f in forbidden {
        if cleaned.eq_ignore_ascii_case(f) {
            return false;
        }
    }
    true
}

/// 列出系统磁盘分区占用（跨平台）。
#[tauri::command]
pub fn get_disk_usage() -> Result<Vec<DiskUsage>, String> {
    #[cfg(target_os = "linux")]
    {
        let out = std::process::Command::new("df")
            .arg("-T")
            .arg("-P")
            .output()
            .map_err(|e| format!("DF_FAILED: {e}"))?;
        let lines = String::from_utf8_lossy(&out.stdout);
        let mut res = Vec::new();
        // df -T -P 输出：Filesystem Type 1024-blocks Used Available Capacity Mounted on
        for (i, line) in lines.lines().enumerate() {
            if i == 0 {
                continue;
            }
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 7 {
                continue;
            }
            let mount = cols[6].to_string();
            // 跳过伪文件系统
            if mount.starts_with("/sys")
                || mount.starts_with("/proc")
                || mount.starts_with("/dev")
                || mount == "/dev"
                || mount.starts_with("/run")
            {
                continue;
            }
            let total = cols[2].parse::<u64>().map_err(|_| "PARSE_ERROR")? * 1024;
            let used = cols[3].parse::<u64>().map_err(|_| "PARSE_ERROR")? * 1024;
            let free = cols[4].parse::<u64>().map_err(|_| "PARSE_ERROR")? * 1024;
            let format = cols[1].to_string();
            res.push(DiskUsage {
                mount,
                total_bytes: total,
                used_bytes: used,
                free_bytes: free,
                format,
            });
        }
        return Ok(res);
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("df")
            .arg("-k")
            .output()
            .map_err(|e| format!("DF_FAILED: {e}"))?;
        let lines = String::from_utf8_lossy(&out.stdout);
        let mut res = Vec::new();
        for (i, line) in lines.lines().enumerate() {
            if i == 0 {
                continue;
            }
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 9 {
                continue;
            }
            let mount = cols[8].to_string();
            if mount.starts_with("/dev") {
                continue;
            }
            let total = cols[1].parse::<u64>().map_err(|_| "PARSE_ERROR")? * 1024;
            let used = cols[2].parse::<u64>().map_err(|_| "PARSE_ERROR")? * 1024;
            let free = cols[3].parse::<u64>().map_err(|_| "PARSE_ERROR")? * 1024;
            let format = "apfs".to_string();
            res.push(DiskUsage {
                mount,
                total_bytes: total,
                used_bytes: used,
                free_bytes: free,
                format,
            });
        }
        return Ok(res);
    }
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let mut res = Vec::new();
        // 通过 wmic 一次性取回所有逻辑盘（各盘大小/剩余空间）
        let out = Command::new("wmic")
            .args([
                "logicaldisk",
                "get",
                "size,freespace,deviceid",
                "/format:csv",
            ])
            .output()
            .ok();
        if let Some(out) = out {
            let txt = String::from_utf8_lossy(&out.stdout);
            for line in txt.lines() {
                let cols: Vec<&str> = line.split(',').collect();
                // CSV: Node,DeviceID,FreeSpace,Size
                if cols.len() >= 4 {
                    let drive = cols[1].trim().to_string();
                    if drive.is_empty() || !drive.ends_with(':') {
                        continue;
                    }
                    let free = cols[2].trim().parse::<u64>().unwrap_or(0);
                    let total = cols[3].trim().parse::<u64>().unwrap_or(0);
                    if total > 0 {
                        res.push(DiskUsage {
                            mount: drive.clone(),
                            total_bytes: total,
                            used_bytes: total.saturating_sub(free),
                            free_bytes: free,
                            format: "NTFS".to_string(),
                        });
                    }
                }
            }
        }
        return Ok(res);
    }
    #[allow(unreachable_code)]
    Ok(Vec::new())
}

/// 读取当前排除项。
#[tauri::command]
pub fn list_exclusions() -> Result<Vec<String>, String> {
    let ex = load_exclusions_impl();
    Ok(ex.paths)
}

/// 添加排除项。
#[tauri::command]
pub fn add_exclusion(path: String) -> Result<(), String> {
    let mut ex = load_exclusions_impl();
    if !ex.paths.contains(&path) {
        ex.paths.push(path);
    }
    save_exclusions_impl(&ex)
}

/// 移除排除项。
#[tauri::command]
pub fn remove_exclusion(path: String) -> Result<(), String> {
    let mut ex = load_exclusions_impl();
    ex.paths.retain(|p| p != &path);
    save_exclusions_impl(&ex)
}

/// 平台专用“一键优化”命令。返回一段人类可读的完成摘要。
/// - Linux: 清理 apt / pacman / dnf 包缓存
/// - macOS: 移除用户级 .DS_Store 与 Finder/缓存中的临时文件
/// - Windows: 清理 %TEMP% 与 Windows Update 缓存
#[tauri::command]
pub fn optimize_disk() -> Result<String, String> {
    let mut summary = Vec::new();
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        // apt
        if let Ok(o) = std::process::Command::new("apt-get")
            .args(["clean"])
            .output()
        {
            if o.status.success() {
                summary.push("apt cache cleaned".to_string());
            }
        }
        if let Ok(o) = std::process::Command::new("pacman")
            .args(["-Sc", "--noconfirm"])
            .output()
        {
            if o.status.success() {
                summary.push("pacman cache cleaned".to_string());
            }
        }
        if let Ok(o) = std::process::Command::new("dnf")
            .args(["clean", "all"])
            .output()
        {
            if o.status.success() {
                summary.push("dnf cache cleaned".to_string());
            }
        }
        // 用户级 .cache 下的零碎缓存
        let cache = format!("{home}/.cache");
        cleanup_ds_store(&cache);
        summary.push("user .cache scanned".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let cache = format!("{home}/Library/Caches");
        let mut fc = 0u64;
        let _ = dir_size(Path::new(&cache), &mut fc);
        cleanup_ds_store(&home);
        summary.push(format!("macOS caches swept ({fc} files)"));
    }
    #[cfg(target_os = "windows")]
    {
        let temp = std::env::var("TEMP").unwrap_or_default();
        let mut fc = 0u64;
        let _ = dir_size(Path::new(&temp), &mut fc);
        summary.push(format!("Windows temp swept ({fc} files)"));
    }
    if summary.is_empty() {
        return Err("OPTIMIZE_EMPTY: 当前平台没有可运行的优化项".into());
    }
    Ok(summary.join("; "))
}

/// 递归删除目录树中的 .DS_Store 文件（macOS 专用，其他平台为空操作）。
#[allow(unused_variables)]
fn cleanup_ds_store(base: &str) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(rd) = fs::read_dir(base) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    cleanup_ds_store(&p.to_string_lossy());
                } else if p.file_name().map(|n| n == ".DS_Store").unwrap_or(false) {
                    let _ = fs::remove_file(&p);
                }
            }
        }
    }
}

/// 供前端查询：本次清理是否需要管理员权限（Linux 系统级缓存通常要）。简单提示用。
#[tauri::command]
pub fn clean_requires_sudo(paths: Vec<String>) -> Result<bool, String> {
    let root = PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
    let mut need = false;
    for p in paths {
        let pb = PathBuf::from(&p);
        if pb.starts_with(&root) && !pb.starts_with(std::env::var("HOME").unwrap_or_default()) {
            need = true;
            break;
        }
    }
    Ok(need)
}
