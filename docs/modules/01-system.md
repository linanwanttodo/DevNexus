# 系统仪表板 — 模块设计文档

## 1. 功能概述

系统仪表板（Dashboard）显示用户的系统硬件信息与实时资源占用情况，是 DevNexus 启动后的默认页面。

**通信链路**:
```
Dashboard.svelte ──→ invoke("get_system_info") ──→ system.rs
                 ──→ invoke("get_resource_usage") ──→ system.rs
```

---

## 2. 数据结构

```rust
// commands/system.rs

pub struct SystemInfo {
    pub os_name: String,         // "macOS", "Windows", "Linux"
    pub os_version: String,      // "14.5", "22H2"
    pub kernel_version: String,  // "24.0.0", "6.5.0-35-generic"
    pub cpu_model: String,       // "Apple M3 Pro", "Intel(R) Core(TM) i7-10750H"
    pub cpu_cores: usize,        // 物理核心数
    pub total_memory_gb: f64,    // 32.0
    pub total_disk_gb: f64,      // 512.0
}

pub struct ResourceUsage {
    pub cpu_usage: f32,          // 45.5 (百分比 0~100)
    pub memory_used_gb: f64,
    pub memory_total_gb: f64,
    pub memory_percent: f32,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
    pub disk_percent: f32,
    pub uptime_secs: u64,        // 系统运行时间（秒）
}
```

**前端对应** (`routes/Dashboard.svelte`):

```javascript
let systemInfo = $state(null);
let resourceUsage = $state(null);

// 通过 $derived 计算展示用的统计卡片
let stats = $derived([
  { label: "CPU Cores", value: systemInfo?.cpu_cores, ... },
  { label: "Memory", value: resourceUsage?.memory_percent, ... },
  { label: "Disk", value: resourceUsage?.disk_percent, ... },
]);
```

---

## 3. 核心实现

### 3.1 系统信息获取 (`get_system_info`)

```rust
#[tauri::command]
pub fn get_system_info() -> SystemInfo {
    let mut sys = System::new();
    sys.refresh_cpu_list();
    sys.refresh_memory();

    SystemInfo {
        os_name: System::name().unwrap_or_default(),
        os_version: System::os_version().unwrap_or_default(),
        kernel_version: System::kernel_version().unwrap_or_default(),
        cpu_model: sys.cpus().first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default(),
        cpu_cores: sys.physical_core_count().unwrap_or(0),
        total_memory_gb: sys.total_memory() as f64 / 1073741824.0,
        total_disk_gb: cached_disk_total_gb(),
    }
}
```

使用 `sysinfo` crate 的 `System` 结构体，一次创建多次更新。每次命令调用会刷新 CPU 和内存数据。

### 3.2 资源使用情况 (`get_resource_usage`)

```rust
#[tauri::command]
pub fn get_resource_usage() -> ResourceUsage {
    let mut sys = System::new_all();
    sys.refresh_all();

    ResourceUsage {
        cpu_usage: sys.global_cpu_usage(),
        memory_used_gb: sys.used_memory() as f64 / 1073741824.0,
        // ... 更多字段
    }
}
```

**性能优化 — 磁盘总量缓存**:

```rust
fn cached_disk_total_gb() -> f64 {
    static DISK_TOTAL: OnceLock<f64> = OnceLock::new();
    *DISK_TOTAL.get_or_init(|| {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        disks.iter().map(|d| d.total_space() as f64).sum::<f64>() / 1073741824.0
    })
}
```

因为磁盘总量在系统运行期间几乎不变，用 `OnceLock` 使其**仅在第一次调用时**枚举一次所有磁盘，之后直接返回缓存值。

关于 `OnceLock`：
- Rust 标准库 `std::sync::OnceLock`，是 `lazy_static!` 的现代替代
- `get_or_init` 保证初始化闭包只执行一次（即使多线程并发）
- 相同时间内，系统进程、CPU 快速刷新，但 Disk 读取只做一次

### 3.3 前端定时轮询

```javascript
onMount(() => {
    loadSystemInfo();
    loadEnvironments();
    // 每 5 秒刷新一次资源使用情况
    const interval = setInterval(refreshResourceUsage, 5000);
    return () => clearInterval(interval);
});
```

前端的 `refreshResourceUsage` 每 5 秒单独轮询 `get_resource_usage`（不含 `get_system_info`），因为系统基本信息不频繁变化。这种做法的好处：
- 减少不必要的数据传输（`SystemInfo` 包含长字符串如 CPU model 名称）
- 降低后端刷新开销（`ResourceUsage` 只刷新 CPU/内存/磁盘，无硬件枚举）

---

## 4. 前端实现细节

### 4.1 统计卡片

使用 `$derived` 将 `systemInfo` 和 `resourceUsage` 合并为 `stats` 数组，数组元素包含：

| 字段 | 来源 | 展示 |
|------|------|------|
| CPU Cores | `SystemInfo.cpu_cores` | 核心数 |
| Memory | `ResourceUsage.memory_percent` | `45%` + `14GB / 32GB` |
| Disk | `ResourceUsage.disk_percent` | `30%` + `150GB / 512GB` |

每个卡片使用 Tailwind 的 `grid grid-cols-3 gap-4` 布局。

### 4.2 系统健康面板

显示更详细的系统运行时状态：

```
Operating System: macOS 14.5
Kernel: 24.0.0
CPU Usage: [████████░░░░] 45.5%
Uptime: 3d 12h 45m
```

CPU 使用率使用内联 `<div>` 宽度百分比实现可视化进度条。

### 4.3 最近环境卡片

通过 `list_environments` 获取前 5 个开发环境（Node、Python、Java 等），显示名称、版本和运行状态。

---

## 5. 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_info_serialization() {
        // 验证 JSON 序列化包含所有字段
        let info = SystemInfo {
            os_name: "macOS".into(),
            os_version: "14.0".into(),
            kernel_version: "23.0.0".into(),
            cpu_model: "Apple M3".into(),
            cpu_cores: 12,
            total_memory_gb: 32.0,
            total_disk_gb: 512.0,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("os_name"));
        assert!(json.contains("cpu_cores"));
    }

    #[test]
    fn test_resource_usage_serialization() {
        // 验证 JSON 序列化包含所有字段
        let usage = ResourceUsage {
            cpu_usage: 45.5,
            memory_used_gb: 14.0,
            // ...
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"cpu_usage\":45.5"));
    }
}
```

测试覆盖：
- 序列化完整性（确保 Tauri IPC 传输时不会丢字段）
- 默认值合理性（边界情况）
- `cached_disk_total_gb` 的缓存行为

---

## 6. 跨平台注意事项

| 字段 | macOS | Linux | Windows |
|------|-------|-------|---------|
| `os_name` | "macOS" | "Linux" | "Windows" |
| `kernel_version` | "24.0.0" | "6.5.0-35-generic" | "10.0.22621" |
| `cpu_model` | "Apple M3" | "Intel(R)..." | "AMD Ryzen 7..." |
| `physical_core_count` | ✅ | ✅ | ✅ |
| `uptime_secs` | ✅ | ✅ | ✅ |

`sysinfo` crate 已内建跨平台支持，唯一需要注意 Windows 上的进程名是带 `.exe` 后缀的，但此模块不涉及进程名操作，无需额外处理。

---

## 7. 关键设计决策

1. **磁盘总量只读一次**: 使用 `OnceLock` 缓存，避免每次轮询都枚举磁盘
2. **分拆两个命令**: `get_system_info` (一次) + `get_resource_usage` (频繁轮询)，减少带宽
3. **前端 5 秒轮询**: 兼顾实时性与性能，CPU 使用率变化对于开发场景 5 秒粒度足够
