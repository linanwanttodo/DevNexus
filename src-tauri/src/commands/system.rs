use serde::Serialize;
use sysinfo::System;

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
        cpu_model: sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_default(),
        cpu_cores: sys.cpus().len(),
        total_memory_gb: (total_memory_gb * 100.0).round() / 100.0,
        total_disk_gb: (total_disk_gb * 100.0).round() / 100.0,
    }
}

#[tauri::command]
pub fn get_resource_usage() -> ResourceUsage {
    let mut sys = System::new_all();
    // 仅刷新 CPU 和内存数据，避免 refresh_all() 刷新磁盘/网络等不必要信息
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

    let disks = sysinfo::Disks::new_with_refreshed_list();
    let disk_total_gb = disks.iter().map(|d| d.total_space() as f64).sum::<f64>() / 1073741824.0;
    let disk_used_gb = disks.iter().map(|d| (d.total_space() - d.available_space()) as f64).sum::<f64>() / 1073741824.0;
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
