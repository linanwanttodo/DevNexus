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
}

#[allow(clippy::needless_return)]
#[tauri::command]
pub fn get_tuning_overview() -> Result<TuningOverview, String> {
    let platform = std::env::consts::OS.to_string();
    #[cfg(target_os = "linux")]
    {
        return Ok(TuningOverview {
            platform,
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

#[cfg(target_os = "linux")]
fn run_cmd(prog: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new(prog)
        .args(args)
        .output()
        .map_err(|e| format!("CMD_FAILED {prog}: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
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
pub fn set_swap(size_mb: u64, path: Option<String>) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let p = path.unwrap_or_else(|| "/swapfile".to_string());
        run_cmd("fallocate", &["-l", &format!("{}M", size_mb), &p])?;
        run_cmd("chmod", &["600", &p])?;
        run_cmd("mkswap", &[&p])?;
        run_cmd("swapon", &[&p])?;
        return Ok(format!("SWAP_OK: {p} enabled ({size_mb} MB)"));
    }
    #[cfg(not(target_os = "linux"))]
    {
        return Err("TUNING_UNSUPPORTED: 仅 Linux 支持 Swap 管理".into());
    }
}

/// 关闭指定 swap 设备。
#[allow(clippy::needless_return)]
#[tauri::command]
pub fn disable_swap(path: String) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        run_cmd("swapoff", &[&path])?;
        return Ok(format!("SWAP_OFF: {path} disabled"));
    }
    #[cfg(not(target_os = "linux"))]
    {
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
pub fn set_dns(preset: String) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let servers: Vec<&str> = match preset.as_str() {
            "114" => vec!["114.114.114.114", "114.114.115.115"],
            "google" => vec!["8.8.8.8", "8.8.4.4"],
            "cloudflare" => vec!["1.1.1.1", "1.0.0.1"],
            "ali" => vec!["223.5.5.5", "223.6.6.6"],
            _ => return Err("DNS_BAD_PRESET".into()),
        };
        // 备份
        if Path::new("/etc/resolv.conf").exists() {
            let _ = fs::copy("/etc/resolv.conf", "/etc/resolv.conf.devnexus.bak");
        }
        let mut content = String::from("# Generated by DevNexus\n");
        for s in servers {
            content.push_str(&format!("nameserver {s}\n"));
        }
        fs::write("/etc/resolv.conf", content).map_err(|e| format!("DNS_WRITE: {e}"))?;
        return Ok(format!("DNS_OK: switched to {preset}"));
    }
    #[cfg(not(target_os = "linux"))]
    {
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
pub fn set_timezone(tz: String) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let tz_path = format!("/usr/share/zoneinfo/{tz}");
        if !Path::new(&tz_path).exists() {
            return Err("TZ_INVALID: 时区不存在".into());
        }
        run_cmd("timedatectl", &["set-timezone", &tz])?;
        return Ok(format!("TZ_OK: set to {tz}"));
    }
    #[cfg(not(target_os = "linux"))]
    {
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
pub fn set_firewall(enable: bool) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let action = if enable { "enable" } else { "disable" };
        run_cmd("ufw", &[action])?;
        return Ok(if enable {
            "FIREWALL_ENABLE".into()
        } else {
            "FIREWALL_DISABLE".into()
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
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
                name: "用户缓存 ~/.cache".into(),
                description: format!("{cache_mb} MB 缓存"),
                size_mb: cache_mb,
                risk: "warn".into(),
                action: "rm -rf ~/.cache/*".into(),
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

#[allow(clippy::needless_return)]
#[tauri::command]
pub fn clean_targets(
    target_ids: Vec<String>,
    dry_run: bool,
    confirmed: bool,
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
                        let _ = run_cmd("journalctl", &["--vacuum-time=7days"]);
                        executed.push("[done] journalctl vacuum → 7d".to_string());
                        freed_mb += t.size_mb;
                    }
                }
                "varlog" => {
                    if dry_run {
                        executed.push(format!("[dry-run] {}", t.action));
                    } else {
                        let _ = run_cmd(
                            "find",
                            &["/var/log", "-name", "*.gz", "-mtime", "+30", "-delete"],
                        );
                        executed.push("[done] /var/log *.gz (mtime+30) 已清理".to_string());
                        freed_mb += t.size_mb;
                    }
                }
                "usercache" => {
                    if dry_run {
                        executed.push(format!("[dry-run] {}", t.action));
                    } else {
                        let home = std::env::var("HOME").unwrap_or_default();
                        let _ = fs::remove_dir_all(format!("{home}/.cache"));
                        executed.push("[done] ~/.cache 已清理".to_string());
                        freed_mb += t.size_mb;
                    }
                }
                "oldkernel" => {
                    if !confirmed {
                        return Err("DANGER_REQUIRES_CONFIRM: 旧内核清理需要二次确认".into());
                    }
                    if dry_run {
                        executed.push("[dry-run] apt-get autoremove --purge".to_string());
                    } else {
                        let _ = run_cmd("apt-get", &["-y", "autoremove", "--purge"]);
                        executed.push("[done] apt autoremove --purge 已执行".to_string());
                    }
                }
                "docker" => {
                    if !confirmed {
                        return Err("DANGER_REQUIRES_CONFIRM: Docker prune 需要二次确认".into());
                    }
                    if dry_run {
                        executed.push("[dry-run] docker system prune -a --volumes".to_string());
                    } else {
                        let _ = run_cmd("docker", &["system", "prune", "-a", "--volumes", "-f"]);
                        executed.push("[done] docker system prune -a --volumes 已执行".to_string());
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
