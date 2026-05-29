use serde::Serialize;
use std::sync::OnceLock;
use sysinfo::System;

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
}

#[derive(Serialize)]
pub struct SystemInfo {
    os_name: String,
    os_version: String,
    kernel_version: String,
    cpu_model: String,
    cpu_cores: usize,
    total_memory_gb: f64,
    total_disk_gb: f64,
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
pub fn get_system_info() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_memory_gb = sys.total_memory() as f64 / 1073741824.0;

    let disks = sysinfo::Disks::new_with_refreshed_list();
    let total_disk_gb = disks.iter().map(|d| d.total_space() as f64).sum::<f64>() / 1073741824.0;

    SystemInfo {
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
        total_disk_gb: (total_disk_gb * 100.0).round() / 100.0,
    }
}

#[tauri::command]
pub fn get_resource_usage() -> ResourceUsage {
    let mut sys = System::new_all();
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

    // 磁盘总量缓存（首次调用时枚举，后续复用），避免每 5 秒枚举磁盘 I/O
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

    ResourceUsage {
        cpu_usage,
        memory_used_gb: (memory_used_gb * 100.0).round() / 100.0,
        memory_total_gb: (memory_total_gb * 100.0).round() / 100.0,
        memory_percent,
        disk_used_gb: (disk_used_gb * 100.0).round() / 100.0,
        disk_total_gb: (disk_total_gb * 100.0).round() / 100.0,
        disk_percent,
        uptime_secs: System::uptime(),
    }
}
