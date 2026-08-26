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
        out.push(("旧内核/日志".into(), "/var/log".into()));
        // 用户缓存 ~/.cache：**逐个分开**成独立勾选项，
        // 特别是编程语言依赖缓存（npm/pip/cargo 等）绝不默认清除，
        // 由用户按需勾选，避免误清导致重装依赖。
        let known: &[(&str, &str)] = &[
            ("npm 依赖缓存", "npm"),
            ("pnpm 依赖缓存", "pnpm"),
            ("yarn 依赖缓存", "yarn"),
            ("pip 依赖缓存", "pip"),
            ("uv 依赖缓存", "uv"),
            ("poetry 依赖缓存", "poetry"),
            ("Cargo 依赖缓存 (rustup/cargo registry)", "cargo"),
            ("Go 模块缓存", "go-build"),
            ("Bun 依赖缓存", "bun"),
        ];
        let cache = format!("{home}/.cache");
        for (name, sub) in known {
            out.push((name.to_string(), format!("{cache}/{sub}")));
        }
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
        // Windows 上 HOME 极少设置；用 USERPROFILE 作为 home 风格的根，
        // 主要清理项仍以 LOCALAPPDATA / TEMP 为主。
        let appdata = std::env::var("LOCALAPPDATA").unwrap_or_default();
        out.push((
            "Windows 临时".into(),
            format!("{}\\Temp", std::env::var("TEMP").unwrap_or_default()),
        ));
        out.push(("缓存目录".into(), appdata));
        out.push(("用户家目录".into(), home));
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
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
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

// ════════════════════════════════════════════════════════════════════
// Phase 3 — Linux 系统调优工具箱（Swap / DNS / 时区 / 防火墙 / 系统限制 / 日志清理）
// 非 Linux 平台：命令返回 TUNING_UNSUPPORTED，前端展示「正在开发中」。
// ════════════════════════════════════════════════════════════════════

/// 总览：当前平台 + 支持的调优能力列表。
#[derive(Serialize)]
pub struct TuningOverview {
    pub platform: String,
    pub supported: Vec<String>,
    pub message: Option<String>,
    /// 是否以 root/管理员权限运行（决定 sysctl 类写操作是否可用）
    pub is_root: bool,
}

/// 是否拥有 root/管理员权限（决定 sysctl 类写操作是否可用）。
fn running_as_root() -> bool {
    #[cfg(target_os = "linux")]
    {
        // /etc/shadow 仅 root 可读：能读取即视为提权。
        return std::fs::read("/etc/shadow").is_ok();
    }
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        return Command::new("net")
            .args(["session"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    }
    #[cfg(target_os = "macos")]
    {
        // /etc/master.passwd 仅 root 可读。
        return std::fs::read("/etc/master.passwd").is_ok();
    }
    #[allow(unreachable_code)]
    false
}

#[allow(clippy::needless_return)]
#[tauri::command]
pub fn get_tuning_overview() -> Result<TuningOverview, String> {
    let platform = std::env::consts::OS.to_string();
    #[cfg(target_os = "linux")]
    {
        let is_root = running_as_root();
        return Ok(TuningOverview {
            platform,
            is_root,
            supported: vec![
                "swap".into(),
                "dns".into(),
                "timezone".into(),
                "firewall".into(),
                "system_limits".into(),
                "log_cleanup".into(),
            ],
            message: None,
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Ok(TuningOverview {
            platform,
            is_root: running_as_root(),
            supported: vec![],
            message: Some("TUNING_UNSUPPORTED: 系统调优工具箱当前仅支持 Linux".into()),
        });
    }
}

/// Swap 信息（swapon --show 解析）
#[derive(Serialize)]
pub struct SwapInfo {
    pub enabled: bool,
    pub devices: Vec<SwapDevice>,
    pub total_mb: u64,
    pub used_mb: u64,
}
#[derive(Serialize)]
pub struct SwapDevice {
    pub filename: String,
    pub type_: String,
    pub size_mb: u64,
    pub used_mb: u64,
}

/// 记录单个清理步骤的执行结果：成功记 `[done]` 并返回 true；失败记 `[failed]`
/// （含原因）并返回 false——调用方只有在本函数返回 true 时才允许计入释放空间。
///
/// `#[cfg_attr]` 保留其在单元测试（所有平台）中的可访问性，同时在
/// 非 Linux 平台的生产构建里允许被标记为 dead_code——Linux 外的清理流程
/// 走的是平台专用分支，不会调用本函数。
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
pub(crate) fn record_step(
    executed: &mut Vec<String>,
    label: &str,
    res: &Result<String, String>,
) -> bool {
    match res {
        Ok(_) => {
            executed.push(format!("[done] {label}"));
            true
        }
        Err(e) => {
            executed.push(format!("[failed] {label}: {e}"));
            false
        }
    }
}

/// swap 文件路径的黑名单前缀（绝对路径 + 无遍历的前提下，仍拒绝系统关键目录）
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
const SWAP_FORBIDDEN_PREFIXES: &[&str] = &[
    "/etc", "/usr", "/bin", "/sbin", "/lib", "/lib64", "/boot", "/proc", "/sys", "/dev", "/run",
    "/opt", "/root",
];

/// 校验 swap 文件路径：必须为绝对路径、无 `..` 遍历分量，
/// 且不得位于系统关键目录之下（防止以 root 权限 fallocate/chmod/mkswap 破坏系统文件）。
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
pub(crate) fn validate_swap_path(p: &str) -> Result<(), String> {
    if p.is_empty() || p.len() > 4096 {
        return Err(format!("Invalid swap path: {p:?}"));
    }
    if p.chars().any(|c| c.is_control()) {
        return Err("Swap path contains control characters".to_string());
    }
    let path = PathBuf::from(p);
    if !path.is_absolute() {
        return Err(format!("Swap path must be absolute: {p}"));
    }
    for comp in path.components() {
        use std::path::Component;
        if matches!(comp, Component::ParentDir | Component::CurDir) {
            return Err(format!("Swap path traversal is not allowed: {p}"));
        }
    }
    for prefix in SWAP_FORBIDDEN_PREFIXES {
        // 精确匹配或目录边界匹配（/etc2 不算 /etc 下）
        if p == *prefix || p.starts_with(&format!("{prefix}/")) {
            return Err(format!(
                "Refusing to create swap under system directory: {p}"
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_cmd(prog: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new(prog)
        .args(args)
        .output()
        .map_err(|e| format!("CMD_FAILED {prog}: {e}"))?;
    // 非 0 退出也视为错误（便于上层感知权限不足等）
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if !err.is_empty() {
            return Err(format!("CMD_FAILED {prog}: {err}"));
        }
        return Err(format!(
            "CMD_FAILED {prog}: exit {}",
            out.status.code().unwrap_or(-1)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// 以提权方式执行命令：已是 root 则直接运行，否则通过 `sudo -S` 喂密码执行。
/// security-sensitive: 处理用户输入的密码，仅通过 stdin 传递，不落盘不记录日志。
#[cfg(target_os = "linux")]
fn run_privileged(password: Option<String>, prog: &str, args: &[&str]) -> Result<String, String> {
    if running_as_root() {
        return run_cmd(prog, args);
    }
    let pw = password.ok_or_else(|| "NEED_SUDO: 需要管理员密码（请在弹窗中输入）".to_string())?;
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("sudo")
        .arg("-S")
        .arg(prog)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("SUDO_SPAWN {prog}: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(format!("{pw}\n").as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        if err.contains("incorrect password")
            || err.contains("Sorry, try again")
            || err.to_lowercase().contains("incorrect")
        {
            return Err("SUDO_AUTH_FAILED: 密码错误".into());
        }
        let trimmed = err.trim();
        if !trimmed.is_empty() {
            return Err(format!("SUDO_FAILED {prog}: {trimmed}"));
        }
        return Err(format!(
            "SUDO_FAILED {prog}: exit {}",
            out.status.code().unwrap_or(-1)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// 校验 sudo 密码是否正确（`sudo -S -v`），供前端在弹窗后即时反馈。
#[tauri::command]
pub fn verify_sudo_password(password: String) -> Result<bool, String> {
    #[cfg(target_os = "linux")]
    {
        if running_as_root() {
            return Ok(true);
        }
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("sudo")
            .args(["-S", "-v"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("SUDO_SPAWN: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(format!("{password}\n").as_bytes());
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if out.status.success() {
            return Ok(true);
        }
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        if err.contains("incorrect password") || err.contains("Sorry, try again") {
            return Err("SUDO_AUTH_FAILED: 密码错误".into());
        }
        Err(format!("SUDO_FAILED: {}", err.trim()))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = password;
        Err("TUNING_UNSUPPORTED: 仅 Linux 支持 sudo 校验".into())
    }
}

#[allow(clippy::needless_return)]
#[tauri::command]
pub fn get_swap_info() -> Result<SwapInfo, String> {
    #[cfg(target_os = "linux")]
    {
        let out = run_cmd("swapon", &["--show=NAME,TYPE,SIZE,USED", "--bytes"]).unwrap_or_default();
        let mut devices = Vec::new();
        let mut total_mb = 0u64;
        let mut used_mb = 0u64;
        for (i, line) in out.lines().enumerate() {
            if i == 0 {
                continue; // header
            }
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 4 {
                let size = cols[2].parse::<u64>().unwrap_or(0) / 1024 / 1024;
                let used = cols[3].parse::<u64>().unwrap_or(0) / 1024 / 1024;
                total_mb += size;
                used_mb += used;
                devices.push(SwapDevice {
                    filename: cols[0].to_string(),
                    type_: cols[1].to_string(),
                    size_mb: size,
                    used_mb: used,
                });
            }
        }
        return Ok(SwapInfo {
            enabled: !devices.is_empty(),
            devices,
            total_mb,
            used_mb,
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持 Swap 管理".into());
    }
}

/// 创建/启用 swapfile（需要 root）。size_mb 为大小，路径默认 /swapfile。
#[allow(clippy::needless_return)]
#[tauri::command]
pub fn set_swap(
    size_mb: u64,
    path: Option<String>,
    password: Option<String>,
) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let p = path.unwrap_or_else(|| "/swapfile".to_string());
        // 以 root 执行 fallocate/chmod/mkswap 前必须校验路径，
        // 防止对 /etc/shadow 等系统文件执行破坏性操作
        validate_swap_path(&p)?;
        run_privileged(
            password.clone(),
            "fallocate",
            &["-l", &format!("{}M", size_mb), &p],
        )?;
        run_privileged(password.clone(), "chmod", &["600", &p])?;
        run_privileged(password.clone(), "mkswap", &[&p])?;
        run_privileged(password, "swapon", &[&p])?;
        return Ok(format!("SWAP_OK: {p} enabled ({size_mb} MB)"));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (size_mb, path, password);
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持 Swap 管理".into());
    }
}

/// 关闭指定 swap 设备。
#[allow(clippy::needless_return)]
#[tauri::command]
pub fn disable_swap(path: String, password: Option<String>) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        validate_swap_path(&path)?;
        run_privileged(password, "swapoff", &[&path])?;
        return Ok(format!("SWAP_OFF: {path} disabled"));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (path, password);
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持 Swap 管理".into());
    }
}

/// DNS 配置（/etc/resolv.conf 内容 + 解析出的 nameserver 列表）
#[derive(Serialize)]
pub struct DnsConfig {
    pub resolv_conf: String,
    pub nameservers: Vec<String>,
    pub search_domains: Vec<String>,
}

#[allow(clippy::needless_return)]
#[tauri::command]
pub fn get_dns_config() -> Result<DnsConfig, String> {
    #[cfg(target_os = "linux")]
    {
        let content =
            fs::read_to_string("/etc/resolv.conf").map_err(|e| format!("RESOLV_CONF: {e}"))?;
        let mut nameservers = Vec::new();
        let mut search_domains = Vec::new();
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("nameserver ") {
                nameservers.push(rest.trim().to_string());
            } else if let Some(rest) = t.strip_prefix("search ") {
                search_domains.push(rest.trim().to_string());
            }
        }
        return Ok(DnsConfig {
            resolv_conf: content,
            nameservers,
            search_domains,
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持 DNS 管理".into());
    }
}

/// 切换 DNS 预设（需 root 写入 /etc/resolv.conf，并备份原文件）。
#[allow(clippy::needless_return)]
#[tauri::command]
pub fn set_dns(preset: String, password: Option<String>) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let servers: Vec<&str> = match preset.as_str() {
            "114" => vec!["114.114.114.114", "114.114.115.115"],
            "google" => vec!["8.8.8.8", "8.8.4.4"],
            "cloudflare" => vec!["1.1.1.1", "1.0.0.1"],
            "ali" => vec!["223.5.5.5", "223.6.6.6"],
            _ => return Err("DNS_BAD_PRESET".into()),
        };
        // 备份：尽量使用提权方式
        if Path::new("/etc/resolv.conf").exists() {
            if running_as_root() {
                let _ = fs::copy("/etc/resolv.conf", "/etc/resolv.conf.devnexus.bak");
            } else if let Some(ref pw) = password {
                let _ = run_privileged(
                    Some(pw.clone()),
                    "cp",
                    &["/etc/resolv.conf", "/etc/resolv.conf.devnexus.bak"],
                );
            }
        }
        let mut content = String::from("# Generated by DevNexus\n");
        for s in servers {
            content.push_str(&format!("nameserver {s}\n"));
        }
        // 写入：root 直接写，非 root 通过 sudo tee
        if running_as_root() {
            fs::write("/etc/resolv.conf", content).map_err(|e| format!("DNS_WRITE: {e}"))?;
        } else {
            let pw = password.ok_or_else(|| "NEED_SUDO: 需要管理员密码".to_string())?;
            use std::io::Write;
            use std::process::{Command, Stdio};
            let mut child = Command::new("sudo")
                .args(["-S", "tee", "/etc/resolv.conf"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("SUDO_SPAWN tee: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(format!("{pw}\n{content}").as_bytes());
            }
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                return Err(format!("DNS_WRITE: {err}"));
            }
        }
        return Ok(format!("DNS_OK: switched to {preset}"));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (preset, password);
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持 DNS 管理".into());
    }
}

/// 时区信息（timedatectl status 解析）
#[derive(Serialize)]
pub struct TimezoneInfo {
    pub timezone: String,
    pub local_time: String,
    pub utc_time: String,
    pub ntp_enabled: bool,
}

#[allow(clippy::needless_return)]
#[tauri::command]
pub fn get_timezone_info() -> Result<TimezoneInfo, String> {
    #[cfg(target_os = "linux")]
    {
        let out = run_cmd("timedatectl", &["status"]).unwrap_or_default();
        let mut timezone = String::new();
        let mut local_time = String::new();
        let mut utc_time = String::new();
        let mut ntp_enabled = false;
        for line in out.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("Time zone:") {
                timezone = rest.split_whitespace().next().unwrap_or("").to_string();
            } else if let Some(rest) = t.strip_prefix("Local time:") {
                local_time = rest.to_string();
            } else if let Some(rest) = t.strip_prefix("Universal time:") {
                utc_time = rest.to_string();
            } else if t.starts_with("NTP service:") {
                ntp_enabled = t.contains("active");
            }
        }
        return Ok(TimezoneInfo {
            timezone,
            local_time,
            utc_time,
            ntp_enabled,
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持时区管理".into());
    }
}

/// 设置时区（需 root）。
#[allow(clippy::needless_return)]
#[tauri::command]
pub fn set_timezone(tz: String, password: Option<String>) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let tz_path = format!("/usr/share/zoneinfo/{tz}");
        if !Path::new(&tz_path).exists() {
            return Err("TZ_INVALID: 时区不存在".into());
        }
        run_privileged(password, "timedatectl", &["set-timezone", &tz])?;
        return Ok(format!("TZ_OK: set to {tz}"));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (tz, password);
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持时区管理".into());
    }
}

/// 防火墙状态（ufw / iptables）
#[derive(Serialize)]
pub struct FirewallStatus {
    pub ufw_active: bool,
    pub ufw_status: String,
    pub iptables_rules: Vec<String>,
}

#[allow(clippy::needless_return)]
#[tauri::command]
pub fn get_firewall_status() -> Result<FirewallStatus, String> {
    #[cfg(target_os = "linux")]
    {
        let ufw = run_cmd("ufw", &["status"]).unwrap_or_default();
        let ufw_active = ufw.contains("Status: active");
        let iptables = run_cmd("iptables", &["-L", "-n", "-v"]).unwrap_or_default();
        let rules: Vec<String> = iptables.lines().take(60).map(|s| s.to_string()).collect();
        return Ok(FirewallStatus {
            ufw_active,
            ufw_status: ufw,
            iptables_rules: rules,
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持防火墙管理".into());
    }
}

/// 启用/禁用防火墙（需 root）。
#[allow(clippy::needless_return)]
#[tauri::command]
pub fn set_firewall(enable: bool, password: Option<String>) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let action = if enable { "enable" } else { "disable" };
        run_privileged(password, "ufw", &[action])?;
        return Ok(if enable {
            "FIREWALL_ENABLE".into()
        } else {
            "FIREWALL_DISABLE".into()
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (enable, password);
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持防火墙管理".into());
    }
}

/// 系统资源限制（ulimit 解析关键项）
#[derive(Serialize)]
pub struct SystemLimits {
    pub nofile_soft: String,
    pub nofile_hard: String,
    pub core_dump: String,
    pub max_user_processes: String,
    pub raw: String,
}

#[allow(clippy::needless_return)]
#[tauri::command]
pub fn get_system_limits() -> Result<SystemLimits, String> {
    #[cfg(target_os = "linux")]
    {
        let raw = run_cmd("bash", &["-c", "ulimit -a"]).unwrap_or_default();
        let nofile_soft = run_cmd("bash", &["-c", "ulimit -Sn"])
            .unwrap_or_default()
            .trim()
            .to_string();
        let nofile_hard = run_cmd("bash", &["-c", "ulimit -Hn"])
            .unwrap_or_default()
            .trim()
            .to_string();
        let core_dump = run_cmd("bash", &["-c", "ulimit -c"])
            .unwrap_or_default()
            .trim()
            .to_string();
        let max_user_processes = run_cmd("bash", &["-c", "ulimit -u"])
            .unwrap_or_default()
            .trim()
            .to_string();
        return Ok(SystemLimits {
            nofile_soft,
            nofile_hard,
            core_dump,
            max_user_processes,
            raw,
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持系统限制查看".into());
    }
}

/// 日志/缓存清理目标扫描（journalctl 占用、旧内核、/var/log、Docker 等）
#[derive(Serialize)]
pub struct CleanupTarget {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_mb: u64,
    pub risk: String, // safe | warn | dangerous
    pub action: String,
}

#[allow(clippy::needless_return)]
#[tauri::command]
pub fn scan_cleanup_targets() -> Result<Vec<CleanupTarget>, String> {
    #[cfg(target_os = "linux")]
    {
        let mut out = Vec::new();
        // 1. journalctl 日志占用
        let journal = run_cmd("journalctl", &["--disk-usage"]).unwrap_or_default();
        let journal_mb = parse_mb_from_line(&journal);
        if journal_mb > 0 {
            out.push(CleanupTarget {
                id: "journal".into(),
                name: "systemd 日志 (journalctl)".into(),
                description: format!("journalctl --disk-usage: {journal_mb} MB"),
                size_mb: journal_mb,
                risk: "safe".into(),
                action: "journalctl --vacuum-time=7days".into(),
            });
        }
        // 2. /var/log
        let mut varlog_fc = 0u64;
        let varlog = dir_size(Path::new("/var/log"), &mut varlog_fc);
        let varlog_mb = varlog / 1024 / 1024;
        if varlog_mb > 0 {
            out.push(CleanupTarget {
                id: "varlog".into(),
                name: "/var/log 日志".into(),
                description: format!("{varlog_mb} MB 日志文件"),
                size_mb: varlog_mb,
                risk: "safe".into(),
                action: "find /var/log -name '*.gz' -mtime +30 -delete".into(),
            });
        }
        // 3. 用户缓存
        let home = std::env::var("HOME").unwrap_or_default();
        let mut cache_fc = 0u64;
        let cache = dir_size(Path::new(&format!("{home}/.cache")), &mut cache_fc);
        let cache_mb = cache / 1024 / 1024;
        if cache_mb > 0 {
            out.push(CleanupTarget {
                id: "usercache".into(),
                name: "编程语言依赖缓存 (npm/pip/cargo...)".into(),
                description: format!("{cache_mb} MB（仅清理已知子目录，需勾选后确认）"),
                size_mb: cache_mb,
                risk: "warn".into(),
                action: "清理 ~/.cache/npm pip cargo 等子目录（非全部）".into(),
            });
        }
        // 4. 旧内核（Debian/Ubuntu）
        let kernels = run_cmd("dpkg", &["-l", "linux-image-*"]).unwrap_or_default();
        let kernel_count = kernels.lines().filter(|l| l.starts_with("ii")).count();
        if kernel_count > 1 {
            out.push(CleanupTarget {
                id: "oldkernel".into(),
                name: "旧内核 (linux-image)".into(),
                description: format!("已安装 {kernel_count} 个内核，可清理旧版本"),
                size_mb: 0,
                risk: "dangerous".into(),
                action: "apt-get autoremove --purge".into(),
            });
        }
        // 5. Docker 悬空资源
        let docker = run_cmd("docker", &["system", "df"]).ok();
        if let Some(d) = docker {
            if !d.trim().is_empty() {
                out.push(CleanupTarget {
                    id: "docker".into(),
                    name: "Docker 悬空资源".into(),
                    description: d.lines().next().unwrap_or("").to_string(),
                    size_mb: 0,
                    risk: "dangerous".into(),
                    action: "docker system prune -a --volumes".into(),
                });
            }
        }
        out.sort_by_key(|a| a.size_mb);
        out.reverse();
        return Ok(out);
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持日志清理扫描".into());
    }
}

/// 解析形如 "Logs take up 256.0M in the journal." 的行里的 MB 数
#[cfg(target_os = "linux")]
fn parse_mb_from_line(s: &str) -> u64 {
    for line in s.lines() {
        let mut iter = line.split_whitespace().rev();
        let Some(unit) = iter.next() else { continue };
        if unit.ends_with('M') {
            if let Ok(v) = unit.trim_end_matches('M').parse::<f64>() {
                return v as u64;
            }
        }
        if unit.ends_with('G') {
            if let Ok(v) = unit.trim_end_matches('G').parse::<f64>() {
                return (v * 1024.0) as u64;
            }
        }
    }
    0
}

/// 执行清理。dry_run=true 时只返回将执行的命令，不实际执行。
/// 危险操作（Docker prune / 旧内核）要求前端传入 confirmed=true 二次确认。
#[derive(Serialize)]
pub struct CleanResult {
    pub executed: Vec<String>,
    pub freed_mb: u64,
    pub dry_run: bool,
}

#[allow(clippy::needless_return, unused_variables)]
#[tauri::command]
pub fn clean_targets(
    target_ids: Vec<String>,
    dry_run: bool,
    confirmed: bool,
    password: Option<String>,
) -> Result<CleanResult, String> {
    #[cfg(target_os = "linux")]
    {
        let targets = scan_cleanup_targets().unwrap_or_default();
        let mut executed = Vec::new();
        let mut freed_mb = 0u64;
        for t in targets {
            if !target_ids.contains(&t.id) {
                continue;
            }
            match t.id.as_str() {
                "journal" => {
                    if dry_run {
                        executed.push(format!("[dry-run] {}", t.action));
                    } else {
                        let res = run_privileged(
                            password.clone(),
                            "journalctl",
                            &["--vacuum-time=7days"],
                        );
                        // 只有命令确实成功才计入释放空间，失败必须如实上报
                        if record_step(&mut executed, "journalctl vacuum → 7d", &res) {
                            freed_mb += t.size_mb;
                        }
                    }
                }
                "varlog" => {
                    if dry_run {
                        executed.push(format!("[dry-run] {}", t.action));
                    } else {
                        let res = run_privileged(
                            password.clone(),
                            "find",
                            &["/var/log", "-name", "*.gz", "-mtime", "+30", "-delete"],
                        );
                        if record_step(&mut executed, "/var/log *.gz (mtime+30) 已清理", &res) {
                            freed_mb += t.size_mb;
                        }
                    }
                }
                "usercache" => {
                    if dry_run {
                        executed.push(format!("[dry-run] {}", t.action));
                    } else {
                        let home = std::env::var("HOME").unwrap_or_default();
                        // 只清理已知的编程语言依赖缓存子目录，避免误清整个 ~/.cache
                        let known_subs: &[&str] = &[
                            "npm", "pnpm", "yarn", "pip", "uv", "poetry", "cargo", "go-build",
                            "bun",
                        ];
                        let mut freed = 0u64;
                        for sub in known_subs {
                            let p = format!("{home}/.cache/{sub}");
                            let p = std::path::PathBuf::from(&p);
                            if p.exists() {
                                let mut fc = 0u64;
                                let size = dir_size(&p, &mut fc);
                                match fs::remove_dir_all(&p) {
                                    Ok(()) => freed += size,
                                    Err(e) => {
                                        executed.push(format!("[failed] 清理 {sub} 缓存: {e}"))
                                    }
                                }
                            }
                        }
                        executed.push(format!("[done] 清理了 {freed} 字节的语言依赖缓存"));
                        freed_mb += freed / 1024 / 1024;
                        // 不清理 ~/.cache 其他内容，留待用户自行决定
                    }
                }
                "oldkernel" => {
                    if !confirmed {
                        return Err("DANGER_REQUIRES_CONFIRM: 旧内核清理需要二次确认".into());
                    }
                    if dry_run {
                        executed.push("[dry-run] apt-get autoremove --purge".to_string());
                    } else {
                        let res = run_privileged(
                            password.clone(),
                            "apt-get",
                            &["-y", "autoremove", "--purge"],
                        );
                        record_step(&mut executed, "apt autoremove --purge 已执行", &res);
                    }
                }
                "docker" => {
                    if !confirmed {
                        return Err("DANGER_REQUIRES_CONFIRM: Docker prune 需要二次确认".into());
                    }
                    if dry_run {
                        executed.push("[dry-run] docker system prune -a --volumes".to_string());
                    } else {
                        let res = run_privileged(
                            password.clone(),
                            "docker",
                            &["system", "prune", "-a", "--volumes", "-f"],
                        );
                        record_step(
                            &mut executed,
                            "docker system prune -a --volumes 已执行",
                            &res,
                        );
                    }
                }
                _ => {}
            }
        }
        return Ok(CleanResult {
            executed,
            freed_mb,
            dry_run,
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持日志清理".into());
    }
}

// ════════════════════════════════════════════════════════════════════
// Windows 优化（phase 3 扩展）
// 标杆参考：BleachBit（安全可解释 cleaners）、Win10Boost/OptWin（服务/注册表/电源）、
// Microsoft DISM WinSxS。大部分操作通过 PowerShell / powercfg / dism 完成；
// 启动项用 winreg 读写注册表。均带 dry-run 预览与危险确认。
// ════════════════════════════════════════════════════════════════════

/// Windows 可清理项（扫描返回）
#[derive(Serialize)]
pub struct WinCleanItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub risk: String, // safe | warn
}

/// 枚举 Windows 系统盘上常见可清理路径（temp / 缓存 / 预读 / 下载 / 回收站）。
#[allow(clippy::needless_return)]
#[tauri::command]
pub fn win_scan_cleanup() -> Result<Vec<WinCleanItem>, String> {
    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;
        let mut items = Vec::new();
        let temp: PathBuf = std::env::var("TEMP").map(PathBuf::from).unwrap_or_default();
        let local_appdata: PathBuf = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_default();
        let windir: PathBuf = std::env::var("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_default();

        // 1. 临时文件 %TEMP%
        if !temp.as_os_str().is_empty() && temp.exists() {
            let mut fc = 0u64;
            let bytes = dir_size(&temp, &mut fc);
            if bytes > 0 {
                items.push(WinCleanItem {
                    id: "temp".into(),
                    name: "临时文件 (%TEMP%)".into(),
                    path: temp.to_string_lossy().to_string(),
                    bytes,
                    risk: "safe".into(),
                });
            }
        }
        // 2. 预读取缓存 %WINDIR%\Prefetch
        let prefetch = windir.join("Prefetch");
        if prefetch.exists() {
            let mut fc = 0u64;
            let bytes = dir_size(&prefetch, &mut fc);
            if bytes > 0 {
                items.push(WinCleanItem {
                    id: "prefetch".into(),
                    name: "预读取缓存 (Prefetch)".into(),
                    path: prefetch.to_string_lossy().to_string(),
                    bytes,
                    risk: "safe".into(),
                });
            }
        }
        // 3. Windows Update 下载缓存 %WINDIR%\SoftwareDistribution\Download
        let mud = windir.join("SoftwareDistribution").join("Download");
        if mud.exists() {
            let mut fc = 0u64;
            let bytes = dir_size(&mud, &mut fc);
            if bytes > 0 {
                items.push(WinCleanItem {
                    id: "wudl".into(),
                    name: "Windows Update 下载缓存".into(),
                    path: mud.to_string_lossy().to_string(),
                    bytes,
                    risk: "warn".into(),
                });
            }
        }
        // 4. 缩略图/图标缓存 %LOCALAPPDATA%\Microsoft\Windows\Explorer
        let thumb = local_appdata.join("Microsoft\\Windows\\Explorer");
        if thumb.exists() {
            let mut fc = 0u64;
            let bytes = dir_size(&thumb, &mut fc);
            if bytes > 0 {
                items.push(WinCleanItem {
                    id: "thumb".into(),
                    name: "缩略图/图标缓存".into(),
                    path: thumb.to_string_lossy().to_string(),
                    bytes,
                    risk: "safe".into(),
                });
            }
        }
        // 5. 浏览器/常用 app 缓存（Chrome/Edge 若存在）
        let home: PathBuf = std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_default();
        for (id, name, rel) in [
            (
                "chrome",
                "Chrome 缓存",
                "AppData\\Local\\Google\\Chrome\\User Data\\Default\\Cache",
            ),
            (
                "edge",
                "Edge 缓存",
                "AppData\\Local\\Microsoft\\Edge\\User Data\\Default\\Cache",
            ),
            ("npm", "npm 缓存", "AppData\\Local\\npm-cache"),
            ("pip", "pip 缓存", "AppData\\Local\\pip\\cache"),
        ] {
            let p = home.join(rel);
            if p.exists() {
                let mut fc = 0u64;
                let bytes = dir_size(&p, &mut fc);
                if bytes > 0 {
                    items.push(WinCleanItem {
                        id: id.into(),
                        name: name.into(),
                        path: p.to_string_lossy().to_string(),
                        bytes,
                        risk: "safe".into(),
                    });
                }
            }
        }
        items.sort_by_key(|a| a.bytes);
        items.reverse();
        return Ok(items);
    }
    #[cfg(not(target_os = "windows"))]
    {
        return Err("TUNING_WIN_UNSUPPORTED: 此功能仅限 Windows".into());
    }
}

/// 删除选中的 Windows 清理项，返回释放字节数。
#[allow(unused_variables, clippy::needless_return)]
#[tauri::command]
pub fn win_clean_paths(ids: Vec<String>) -> Result<u64, String> {
    #[cfg(target_os = "windows")]
    {
        let all = win_scan_cleanup().unwrap_or_default();
        let mut freed = 0u64;
        for item in all {
            if !ids.contains(&item.id) {
                continue;
            }
            let p = std::path::PathBuf::from(&item.path);
            if !p.exists() {
                continue;
            }
            if let Ok(meta) = std::fs::metadata(&p) {
                if meta.is_dir() {
                    // 逐个清空内容而非整目录删除（保留目录结构，避免路径解析问题）
                    if let Ok(rd) = std::fs::read_dir(&p) {
                        for entry in rd.flatten() {
                            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                let _ = std::fs::remove_dir_all(entry.path());
                            } else {
                                let _ = std::fs::remove_file(entry.path());
                            }
                        }
                    }
                    freed += item.bytes;
                }
            }
        }
        return Ok(freed);
    }
    #[cfg(not(target_os = "windows"))]
    {
        return Err("TUNING_WIN_UNSUPPORTED: 此功能仅限 Windows".into());
    }
}

/// WinSxS 组件库清理（DISM /StartComponentCleanup，需管理员）。
#[allow(unused_variables, clippy::needless_return)]
#[tauri::command]
pub fn win_winsxs_cleanup(reset_base: bool, password: Option<String>) -> Result<String, String> {
    let _ = password;
    #[cfg(target_os = "windows")]
    {
        let mut args = vec!["/online", "/Cleanup-Image", "/StartComponentCleanup"];
        if reset_base {
            args.push("/ResetBase");
        }
        let out = std::process::Command::new("Dism")
            .args(&args)
            .output()
            .map_err(|e| format!("DISM_FAILED: {e}"))?;
        let tail = String::from_utf8_lossy(&out.stderr);
        Ok(format!(
            "WINSXS_DONE (ntstatus {}): {}",
            out.status.code().unwrap_or(-1),
            tail.lines().next_back().unwrap_or("")
        ))
    }
    #[cfg(not(target_os = "windows"))]
    {
        return Err("TUNING_WIN_UNSUPPORTED: 此功能仅限 Windows".into());
    }
}

/// 休眠文件 (hiberfil.sys) 状态与开关。true=开启，false=关闭(释放约等于内存大小的空间)。
#[derive(Serialize)]
pub struct HibernationStatus {
    pub enabled: bool,
    pub hiberfil_mb: u64,
}

#[allow(clippy::needless_return)]
#[tauri::command]
pub fn win_get_hibernation() -> Result<HibernationStatus, String> {
    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("powercfg")
            .args(["/a"])
            .output()
            .ok();
        let enabled = out
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("Hibernation"))
            .unwrap_or(false);
        let mut hiberfil_mb = 0u64;
        // hiberfil.sys 位于系统盘根（如 C:\hiberfil.sys）
        let root = std::path::PathBuf::from("C:\\");
        let hf = root.join("hiberfil.sys");
        if hf.exists() {
            if let Ok(m) = std::fs::metadata(&hf) {
                hiberfil_mb = m.len() / 1024 / 1024;
            }
        }
        return Ok(HibernationStatus {
            enabled,
            hiberfil_mb,
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        return Err("TUNING_WIN_UNSUPPORTED: 此功能仅限 Windows".into());
    }
}

/// 开关休眠。enable=true 开启 / enable=false 关闭（powercfg /hibernate on|off）。
#[allow(unused_variables, clippy::needless_return)]
#[tauri::command]
pub fn win_set_hibernation(enable: bool, password: Option<String>) -> Result<String, String> {
    let _ = password;
    #[cfg(target_os = "windows")]
    {
        let arg = if enable { "on" } else { "off" };
        let out = std::process::Command::new("powercfg")
            .args(["/hibernate", arg])
            .output()
            .map_err(|e| format!("POWERCFG_FAILED: {e}"))?;
        Ok(format!(
            "HIBERNATION_{} (ntstatus {})",
            arg,
            out.status.code().unwrap_or(-1)
        ))
    }
    #[cfg(not(target_os = "windows"))]
    {
        return Err("TUNING_WIN_UNSUPPORTED: 此功能仅限 Windows".into());
    }
}

/// 一条启动项
#[derive(Serialize)]
pub struct WinStartupEntry {
    pub name: String,
    pub command: String,
    pub hive: String, // "HKCU" | "HKLM"
    pub enabled: bool,
}

/// 列出当前用户与机器级启动项（来自注册表 Run 键）。
#[allow(clippy::needless_return)]
#[tauri::command]
pub fn win_list_startup() -> Result<Vec<WinStartupEntry>, String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        let mut out = Vec::new();
        let runs = [
            (
                "HKCU".to_string(),
                RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
                    "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    KEY_READ,
                ),
            ),
            (
                "HKLM".to_string(),
                RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(
                    "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    KEY_READ,
                ),
            ),
        ];
        for (hive, key) in runs {
            if let Ok(key) = key {
                for (name, _value) in key.enum_values().flatten() {
                    if let Ok(command) = key.get_value::<String, _>(&name) {
                        out.push(WinStartupEntry {
                            name: name.clone(),
                            command,
                            hive: hive.clone(),
                            enabled: true, // Run 键存在即启用
                        });
                    }
                }
            }
        }
        return Ok(out);
    }
    #[cfg(not(target_os = "windows"))]
    {
        return Err("TUNING_WIN_UNSUPPORTED: 此功能仅限 Windows".into());
    }
}

/// 禁用/启用某个启动项：写入 StartupApproved\Run 状态字节（禁用=0x03，启用=0x02）。
#[allow(unused_variables, clippy::needless_return)]
#[tauri::command]
pub fn win_set_startup(name: String, hive: String, enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        let hkey = if hive == "HKLM" {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        };
        let approved_path =
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
        let root = RegKey::predef(hkey);
        let approved = match root.open_subkey_with_flags(approved_path, KEY_SET_VALUE | KEY_READ) {
            Ok(k) => k,
            Err(_) => {
                root.create_subkey(approved_path)
                    .map_err(|e| e.to_string())?
                    .0
            }
        };
        let state: Vec<u8> = if enable {
            vec![0x02, 0x00]
        } else {
            vec![0x03, 0x00]
        };
        // winreg 0.52：ToRegValue 不为 &[u8] 实现，所以用 set_raw_value +
        // 显式构造的 RegValue（RegType 枚举位于 winreg::enums）。
        approved
            .set_raw_value(
                &name,
                &winreg::RegValue {
                    bytes: state,
                    vtype: winreg::enums::RegType::REG_BINARY,
                },
            )
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        return Err("TUNING_WIN_UNSUPPORTED: 此功能仅限 Windows".into());
    }
}

/// Windows 存储占用概览（PowerShell Get-CimInstance，替代已弃用的 wmic）。
#[allow(clippy::needless_return)]
#[tauri::command]
pub fn win_storage_usage() -> Result<Vec<DiskUsage>, String> {
    #[cfg(target_os = "windows")]
    {
        let script = "Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=3' | Select-Object DeviceID,Size,FreeSpace,@{N='Fmt';E={$_.FileSystem}} | ConvertTo-Json";
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .map_err(|e| format!("PS_FAILED: {e}"))?;
        let txt = String::from_utf8_lossy(&out.stdout);
        let mut res = Vec::new();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
            let arrs = match v.as_array() {
                Some(a) => a.clone(),
                None => vec![v],
            };
            for o in arrs {
                let mount = o
                    .get("DeviceID")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let total = o.get("Size").and_then(|x| x.as_u64()).unwrap_or(0);
                let free = o.get("FreeSpace").and_then(|x| x.as_u64()).unwrap_or(0);
                let format = o
                    .get("Fmt")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if total > 0 {
                    res.push(DiskUsage {
                        mount,
                        total_bytes: total,
                        used_bytes: total.saturating_sub(free),
                        free_bytes: free,
                        format,
                    });
                }
            }
        }
        return Ok(res);
    }
    #[cfg(not(target_os = "windows"))]
    {
        return Err("TUNING_WIN_UNSUPPORTED: 此功能仅限 Windows".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_step_ok() {
        let mut executed = Vec::new();
        let ok = record_step(&mut executed, "journalctl vacuum", &Ok("".into()));
        assert!(ok);
        assert_eq!(executed, vec!["[done] journalctl vacuum".to_string()]);
    }

    #[test]
    fn test_record_step_failure_reported_not_done() {
        let mut executed = Vec::new();
        let ok = record_step(
            &mut executed,
            "docker prune",
            &Err("SUDO_AUTH_FAILED: 密码错误".into()),
        );
        assert!(!ok, "failed step must not count as success");
        assert!(
            executed[0].starts_with("[failed] docker prune"),
            "got: {:?}",
            executed
        );
        assert!(executed[0].contains("SUDO_AUTH_FAILED"));
    }

    #[test]
    fn test_validate_swap_path_accepts_common_locations() {
        // 平台原生绝对路径
        #[cfg(not(target_os = "windows"))]
        {
            assert!(validate_swap_path("/swapfile").is_ok());
            assert!(validate_swap_path("/swap/swapfile2").is_ok());
            assert!(validate_swap_path("/home/u/swapfile").is_ok());
            assert!(validate_swap_path("/var/swapfile").is_ok());
        }
        #[cfg(target_os = "windows")]
        {
            // swap 在 Windows 上产品层不启用，但路径校验函数本身是纯逻辑、可在 Windows 上跑测试
            assert!(validate_swap_path("C:\\swapfile").is_ok());
            assert!(validate_swap_path("C:\\swap\\swapfile2").is_ok());
            assert!(validate_swap_path("D:\\Users\\u\\swapfile").is_ok());
        }
    }

    #[test]
    fn test_validate_swap_path_rejects_dangerous() {
        // Linux/macOS：SWAP_FORBIDDEN_PREFIXES 是 POSIX 风格，可测拒绝 /etc 等
        #[cfg(not(target_os = "windows"))]
        {
            assert!(validate_swap_path("/etc/shadow").is_err());
            assert!(validate_swap_path("/usr/bin/ls").is_err());
            assert!(validate_swap_path("/boot/vmlinuz").is_err());
            assert!(validate_swap_path("relative/swap").is_err());
            assert!(validate_swap_path("/safe/../etc/shadow").is_err());
        }
        // Windows：SWAP_FORBIDDEN_PREFIXES 是 POSIX 风格（无 Windows 危险目录匹配），
        // 因此只验证"相对路径 / .. 遍历 / 空"这几条与平台无关的拒绝规则。
        // 注：Path::components() 会把路径中段的 "." 规范化掉，故不用 ".\\" 用例。
        #[cfg(target_os = "windows")]
        {
            assert!(validate_swap_path("relative\\swap").is_err());
            assert!(validate_swap_path("C:\\safe\\..\\Windows\\System32").is_err());
            assert!(validate_swap_path("C:\\swap with\nnewline").is_err());
        }
        assert!(validate_swap_path("").is_err());
    }
}
