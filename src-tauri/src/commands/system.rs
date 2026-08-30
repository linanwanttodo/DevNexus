use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};
use sysinfo::System;

/// 平台相关的系统硬件信息（温度 / GPU / 电池）
#[derive(Serialize, Clone)]
pub struct HardwareStatus {
    /// CPU 最高温度（℃），无传感器时为空
    pub cpu_temp_c: Option<f32>,
    /// GPU 名称（nvidia-smi），未检测到 NVIDIA GPU 时为空
    pub gpu_name: Option<String>,
    /// GPU 显存使用量（MB）
    pub gpu_memory_used_mb: Option<u64>,
    /// GPU 显存总量（MB）
    pub gpu_memory_total_mb: Option<u64>,
    /// GPU 使用率（%）
    pub gpu_usage_percent: Option<f32>,
    /// GPU 温度（℃）
    pub gpu_temp_c: Option<f32>,
    /// 电池电量（%），非电池供电设备为空
    pub battery_percent: Option<f32>,
    /// 电池状态（Charging / Discharging / Full / ...）
    pub battery_status: Option<String>,
}

/// 获取系统信息单例（避免每 5 秒重新分配 sysinfo 内部结构）
fn system() -> &'static Mutex<System> {
    static SYS: OnceLock<Mutex<System>> = OnceLock::new();
    SYS.get_or_init(|| {
        let mut sys = System::new_all();
        sys.refresh_all();
        Mutex::new(sys)
    })
}

/// 获取当前应用版本号（编译时从 Cargo.toml 读取）
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// 缓存磁盘总量（GB），避免每次 get_resource_usage 都枚举磁盘
fn cached_disk_total_gb() -> f64 {
    static DISK_TOTAL: OnceLock<f64> = OnceLock::new();
    *DISK_TOTAL.get_or_init(|| {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        disks.iter().map(|d| d.total_space() as f64).sum::<f64>() / 1073741824.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_info_serialization() {
        let info = SystemInfo {
            os_name: "TestOS".to_string(),
            os_version: "1.0".to_string(),
            kernel_version: "6.0".to_string(),
            cpu_model: "Test CPU".to_string(),
            cpu_cores: 4,
            total_memory_gb: 16.0,
            total_disk_gb: 512.0,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("TestOS"));
        assert!(json.contains("\"cpu_cores\":4"));
        assert!(json.contains("\"total_memory_gb\":16.0"));
    }

    #[test]
    fn test_resource_usage_serialization() {
        let usage = ResourceUsage {
            cpu_usage: 45.5,
            memory_used_gb: 8.0,
            memory_total_gb: 16.0,
            memory_percent: 50.0,
            disk_used_gb: 200.0,
            disk_total_gb: 512.0,
            disk_percent: 39.0,
            uptime_secs: 3600,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"cpu_usage\":45.5"));
        assert!(json.contains("\"uptime_secs\":3600"));
    }

    #[test]
    fn test_system_info_default_fields() {
        let info = SystemInfo {
            os_name: String::new(),
            os_version: String::new(),
            kernel_version: String::new(),
            cpu_model: String::new(),
            cpu_cores: 0,
            total_memory_gb: 0.0,
            total_disk_gb: 0.0,
        };
        assert_eq!(info.cpu_cores, 0);
        assert_eq!(info.total_memory_gb, 0.0);
    }

    #[test]
    fn test_resource_usage_zero_values() {
        let usage = ResourceUsage {
            cpu_usage: 0.0,
            memory_used_gb: 0.0,
            memory_total_gb: 0.0,
            memory_percent: 0.0,
            disk_used_gb: 0.0,
            disk_total_gb: 0.0,
            disk_percent: 0.0,
            uptime_secs: 0,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"cpu_usage\":0.0"));
        assert!(json.contains("\"memory_percent\":0.0"));
    }

    #[test]
    fn test_hardware_status_serialization() {
        let status = HardwareStatus {
            cpu_temp_c: Some(52.5),
            gpu_name: Some("NVIDIA GeForce RTX 3060".to_string()),
            gpu_memory_used_mb: Some(1024),
            gpu_memory_total_mb: Some(12288),
            gpu_usage_percent: Some(35.0),
            gpu_temp_c: Some(61.0),
            battery_percent: Some(87.0),
            battery_status: Some("Charging".to_string()),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"cpu_temp_c\":52.5"));
        assert!(json.contains("\"gpu_name\":\"NVIDIA GeForce RTX 3060\""));
        assert!(json.contains("\"gpu_memory_used_mb\":1024"));
        assert!(json.contains("\"battery_status\":\"Charging\""));
    }

    #[test]
    fn test_hardware_status_optional_fields() {
        // 台式机无电池、无 NVIDIA GPU 时全部字段应序列化为 null，而不是报错
        let status = HardwareStatus {
            cpu_temp_c: None,
            gpu_name: None,
            gpu_memory_used_mb: None,
            gpu_memory_total_mb: None,
            gpu_usage_percent: None,
            gpu_temp_c: None,
            battery_percent: None,
            battery_status: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"cpu_temp_c\":null"));
        assert!(json.contains("\"gpu_name\":null"));
        assert!(json.contains("\"battery_percent\":null"));
    }

    #[test]
    fn test_cpu_usage_after_second_refresh() {
        // 模拟 get_resource_usage 的调用序列：初始化采样 → refresh → 读 usage
        let mut sys = System::new_all();
        sys.refresh_all();
        std::thread::sleep(std::time::Duration::from_millis(300));
        sys.refresh_cpu_specifics(sysinfo::CpuRefreshKind::everything());
        let usage = sys.global_cpu_usage();
        tracing::debug!(cpu_usage = %usage, "CPU usage after 2 samples");
        // 只验证调用不 panic 且返回有限值，不强行断言非零（空闲机器可能接近 0）
        assert!(usage.is_finite() && usage >= 0.0);
    }

    #[test]
    fn test_hardware_status_optional_fields_none() {
        // 测试所有可选字段都为 None 的情况（台式机场景）
        let status = HardwareStatus {
            cpu_temp_c: None,
            gpu_name: None,
            gpu_memory_used_mb: None,
            gpu_memory_total_mb: None,
            gpu_usage_percent: None,
            gpu_temp_c: None,
            battery_percent: None,
            battery_status: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        // 所有字段都应该序列化为 null
        assert!(json.contains("\"cpu_temp_c\":null"));
        assert!(json.contains("\"gpu_name\":null"));
        assert!(json.contains("\"battery_percent\":null"));
        assert!(json.contains("\"battery_status\":null"));
    }

    #[test]
    fn test_system_info_large_values() {
        // 测试大数值是否正确序列化
        let info = SystemInfo {
            os_name: "Ubuntu".to_string(),
            os_version: "24.04 LTS".to_string(),
            kernel_version: "6.8.0-45-generic".to_string(),
            cpu_model: "AMD Ryzen 9 7950X 16-Core Processor".to_string(),
            cpu_cores: 32,
            total_memory_gb: 128.0,
            total_disk_gb: 4096.0,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"cpu_cores\":32"));
        assert!(json.contains("\"total_memory_gb\":128.0"));
        assert!(json.contains("\"total_disk_gb\":4096.0"));
    }

    #[test]
    fn test_resource_usage_edge_cases() {
        // 测试边界值
        let usage = ResourceUsage {
            cpu_usage: 100.0, // 满载
            memory_used_gb: 64.0,
            memory_total_gb: 64.0, // 内存用满
            memory_percent: 100.0,
            disk_used_gb: 999.0,
            disk_total_gb: 1000.0, // 磁盘几乎满了
            disk_percent: 99.9,
            uptime_secs: u64::MAX, // 最大运行时间
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"cpu_usage\":100.0"));
        assert!(json.contains("\"memory_percent\":100.0"));
        assert!(json.contains("\"disk_percent\":99.9"));
    }

    #[test]
    fn test_get_app_version() {
        // 测试版本号获取
        let version = get_app_version();
        assert!(!version.is_empty());
        // 版本号应该符合 semver 格式
        assert!(version.matches('.').count() >= 2 || version.contains('-'));
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SystemInfo {
    os_name: String,
    os_version: String,
    kernel_version: String,
    cpu_model: String,
    cpu_cores: usize,
    total_memory_gb: f64,
    total_disk_gb: f64,
}

// ── 静态系统信息本地缓存 ──
// 这些信息（OS/CPU/内存总量/磁盘总量）在运行期几乎不变，却每次调用都要
// 枚举磁盘 + 读系统文件。写入 data_dir 缓存后，24 小时内启动直接读文件，
// 不再重复执行采集命令，显著加快应用启动与概览页加载。
const SYSTEM_INFO_CACHE_TTL_SECS: u64 = 24 * 3600;
const SYSTEM_INFO_CACHE_FILE: &str = "system_info_cache.json";

#[derive(Serialize, Deserialize)]
struct SystemInfoCache {
    fetched_at_secs: u64,
    info: SystemInfo,
}

fn system_info_cache_path() -> std::path::PathBuf {
    crate::utils::data_dir().join(SYSTEM_INFO_CACHE_FILE)
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 读取未过期的缓存；不存在 / 过期 / 损坏时返回 None（调用方重新采集）
fn load_cached_system_info() -> Option<SystemInfo> {
    let text = std::fs::read_to_string(system_info_cache_path()).ok()?;
    let cache: SystemInfoCache = serde_json::from_str(&text).ok()?;
    if now_unix_secs().saturating_sub(cache.fetched_at_secs) > SYSTEM_INFO_CACHE_TTL_SECS {
        return None;
    }
    Some(cache.info)
}

fn save_system_info_cache(info: &SystemInfo) {
    let cache = SystemInfoCache {
        fetched_at_secs: now_unix_secs(),
        info: info.clone(),
    };
    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = std::fs::write(system_info_cache_path(), json);
    }
}

/// 安全获取系统单例锁：即使锁因某线程 panic 而中毒，也恢复继续使用，
/// 而不是返回 "internal lock poisoned" 让概览页报错。
fn lock_system() -> Result<MutexGuard<'static, System>, String> {
    match system().lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            tracing::warn!("[DevNexus] system info mutex was poisoned; recovering");
            Ok(poisoned.into_inner())
        }
    }
}

#[derive(Serialize)]
pub struct ResourceUsage {
    cpu_usage: f32,
    memory_used_gb: f64,
    memory_total_gb: f64,
    memory_percent: f32,
    disk_used_gb: f64,
    disk_total_gb: f64,
    disk_percent: f32,
    uptime_secs: u64,
}

#[tauri::command]
pub fn get_system_info() -> Result<SystemInfo, String> {
    // 静态信息缓存命中则直接返回，避免每次启动都枚举磁盘/读系统文件。
    // 热路径（Dashboard 挂载 / 轮询共享）不打 stderr 日志，统一走 tracing::debug!
    if let Some(cached) = load_cached_system_info() {
        tracing::debug!("[DevNexus] get_system_info: cache hit");
        return Ok(cached);
    }
    tracing::debug!("[DevNexus] get_system_info: cache miss, collecting...");
    let result = collect_system_info();
    match &result {
        Ok(info) => {
            save_system_info_cache(info);
            tracing::debug!("[DevNexus] get_system_info: ok, cache saved");
        }
        Err(e) => tracing::error!("[DevNexus] get_system_info: ERROR: {e}"),
    }
    result
}

fn collect_system_info() -> Result<SystemInfo, String> {
    // 磁盘枚举在锁外完成，避免 I/O 占用全局锁
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let total_disk_gb =
        (disks.iter().map(|d| d.total_space() as f64).sum::<f64>() / 1073741824.0 * 100.0).round()
            / 100.0;

    let mut sys = lock_system()?;
    // 只刷新内存。cpu_model / cpu_cores 在单例初始化时已填充；
    // 用 refresh_all 会重置 CPU 占用率采样基准，导致紧随其后的
    // get_resource_usage 在毫秒级间隔内读到 ~0% 占用。
    sys.refresh_memory();

    let total_memory_gb = sys.total_memory() as f64 / 1073741824.0;

    Ok(SystemInfo {
        os_name: System::name().unwrap_or_default(),
        os_version: System::os_version().unwrap_or_default(),
        kernel_version: System::kernel_version().unwrap_or_default(),
        cpu_model: sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default(),
        cpu_cores: sys.cpus().len(),
        total_memory_gb: (total_memory_gb * 100.0).round() / 100.0,
        total_disk_gb,
    })
}

#[tauri::command]
pub fn get_resource_usage() -> Result<ResourceUsage, String> {
    // 磁盘枚举在锁外完成，避免 I/O 占用全局锁
    let disk_total_gb = cached_disk_total_gb();
    // 从缓存的总量反算使用量：disk_total - 所有磁盘剩余空间之和
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let disk_used_gb = disk_total_gb
        - disks
            .iter()
            .map(|d| d.available_space() as f64)
            .sum::<f64>()
            / 1073741824.0;
    let disk_percent = if disk_total_gb > 0.0 {
        (disk_used_gb / disk_total_gb * 100.0) as f32
    } else {
        0.0
    };

    let mut sys = lock_system()?;
    sys.refresh_cpu_specifics(sysinfo::CpuRefreshKind::everything());
    sys.refresh_memory();

    let memory_total_gb = sys.total_memory() as f64 / 1073741824.0;
    let memory_used_gb = sys.used_memory() as f64 / 1073741824.0;
    let memory_percent = if sys.total_memory() > 0 {
        (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0
    } else {
        0.0
    };

    let cpu_usage = sys.global_cpu_usage();

    Ok(ResourceUsage {
        cpu_usage,
        memory_used_gb: (memory_used_gb * 100.0).round() / 100.0,
        memory_total_gb: (memory_total_gb * 100.0).round() / 100.0,
        memory_percent,
        disk_used_gb: (disk_used_gb * 100.0).round() / 100.0,
        disk_total_gb: (disk_total_gb * 100.0).round() / 100.0,
        disk_percent,
        uptime_secs: System::uptime(),
    })
}

// ---------- 硬件状态（温度 / GPU / 电池） ----------

/// 收集硬件状态。各部分独立容错：某一部分失败不影响其他部分。
#[tauri::command]
pub fn get_hardware_status() -> Result<HardwareStatus, String> {
    // 统一缓存 30s：GPU 探测（nvidia-smi 子进程）与电池探测是高成本调用，
    // CPU 温度对秒级精度无需求；共享一份 TTL 避免多次锁定/判断分支
    static CACHE: OnceLock<Mutex<Option<(std::time::Instant, HardwareStatus)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((at, cached)) = guard.as_ref() {
            if at.elapsed().as_secs() < 30 {
                // 克隆缓存中的状态（HardwareStatus 需 Clone）
                return Ok(HardwareStatus {
                    cpu_temp_c: cached.cpu_temp_c,
                    gpu_name: cached.gpu_name.clone(),
                    gpu_memory_used_mb: cached.gpu_memory_used_mb,
                    gpu_memory_total_mb: cached.gpu_memory_total_mb,
                    gpu_usage_percent: cached.gpu_usage_percent,
                    gpu_temp_c: cached.gpu_temp_c,
                    battery_percent: cached.battery_percent,
                    battery_status: cached.battery_status.clone(),
                });
            }
        }
    }
    let status = HardwareStatus {
        cpu_temp_c: read_cpu_temperature(),
        gpu_name: None,
        gpu_memory_used_mb: None,
        gpu_memory_total_mb: None,
        gpu_usage_percent: None,
        gpu_temp_c: None,
        battery_percent: None,
        battery_status: None,
    };
    let (gpu_name, gpu_memory_used_mb, gpu_memory_total_mb, gpu_usage_percent, gpu_temp_c) =
        read_gpu_status();
    let (battery_percent, battery_status) = read_battery_status();
    let status = HardwareStatus {
        gpu_name,
        gpu_memory_used_mb,
        gpu_memory_total_mb,
        gpu_usage_percent,
        gpu_temp_c,
        battery_percent,
        battery_status,
        ..status
    };
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((
            std::time::Instant::now(),
            HardwareStatus {
                cpu_temp_c: status.cpu_temp_c,
                gpu_name: status.gpu_name.clone(),
                gpu_memory_used_mb: status.gpu_memory_used_mb,
                gpu_memory_total_mb: status.gpu_memory_total_mb,
                gpu_usage_percent: status.gpu_usage_percent,
                gpu_temp_c: status.gpu_temp_c,
                battery_percent: status.battery_percent,
                battery_status: status.battery_status.clone(),
            },
        ));
    }
    Ok(status)
}

/// 通过 sysinfo Components 读取 CPU 温度（Linux hwmon / Windows），取最高值
fn read_cpu_temperature() -> Option<f32> {
    sysinfo::Components::new_with_refreshed_list()
        .iter()
        .filter_map(|c| c.temperature())
        .filter(|t| t.is_finite() && *t > 0.0)
        .reduce(f32::max)
}

/// (GPU 名称, 显存已用 MB, 显存总量 MB, 利用率 %, 温度 ℃)
type GpuStatus = (
    Option<String>,
    Option<u64>,
    Option<u64>,
    Option<f32>,
    Option<f32>,
);

/// 通过 nvidia-smi 读取 NVIDIA GPU 显存/使用率/温度，未安装或非 NVIDIA 时返回 None
fn read_gpu_status() -> GpuStatus {
    let smi = match which::which("nvidia-smi") {
        Ok(path) => path,
        Err(_) => return (None, None, None, None, None),
    };
    let out = match Command::new(smi)
        .args([
            "--query-gpu=name,memory.used,memory.total,utilization.gpu,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => return (None, None, None, None, None),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    // 多 GPU 时取第一行
    let Some(line) = text.lines().find(|l| !l.trim().is_empty()) else {
        return (None, None, None, None, None);
    };
    let mut parts = line.split(',').map(|s| s.trim()).collect::<Vec<_>>();
    if parts.is_empty() {
        return (None, None, None, None, None);
    }
    let name = Some(parts.remove(0).to_string());
    let mem_used = parts.first().and_then(|s| s.parse::<u64>().ok());
    let mem_total = parts.get(1).and_then(|s| s.parse::<u64>().ok());
    let usage = parts.get(2).and_then(|s| s.parse::<f32>().ok());
    let temp = parts.get(3).and_then(|s| s.parse::<f32>().ok());
    (name, mem_used, mem_total, usage, temp)
}

/// 读取电池状态。Linux 走 sysfs；macOS 通过 pmset；不支持时返回 None
fn read_battery_status() -> (Option<f32>, Option<String>) {
    #[cfg(target_os = "linux")]
    {
        let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") else {
            return (None, None);
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(String::from) else {
                continue;
            };
            if !name.starts_with("BAT") {
                continue;
            }
            let capacity = std::fs::read_to_string(entry.path().join("capacity"))
                .ok()
                .and_then(|s| s.trim().parse::<f32>().ok());
            let status = std::fs::read_to_string(entry.path().join("status"))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s != "Unknown");
            if capacity.is_some() || status.is_some() {
                return (capacity, status);
            }
        }
        (None, None)
    }

    #[cfg(target_os = "macos")]
    {
        let out = Command::new("pmset").arg("-g").arg("batt").output().ok();
        let Some(out) = out.filter(|o| o.status.success()) else {
            return (None, None);
        };
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        // 输出形如: -InternalBattery-0 (id=...) 83%; charging; 0:30 remaining
        let percent = text
            .split(';')
            .next()
            .and_then(|s| s.split('%').next())
            .and_then(|s| s.split_whitespace().last())
            .and_then(|s| s.parse::<f32>().ok());
        let status = if text.contains("charging") {
            Some("Charging".to_string())
        } else if text.contains("discharging") {
            Some("Discharging".to_string())
        } else if text.contains("charged") {
            Some("Full".to_string())
        } else {
            None
        };
        if percent.is_some() || status.is_some() {
            return (percent, status);
        }
        (None, None)
    }

    // Windows 不支持（需电池 API），返回 None
    #[cfg(target_os = "windows")]
    {
        (None, None)
    }
}
