# 系统调优 — 模块设计文档

## 1. 功能概述

系统调优（System Tune）为 DevNexus 提供磁盘空间分析与垃圾清理能力，以及平台专用的
一键优化。侧边栏以「系统调优」为入口，包含三个子页：磁盘清理、macOS 优化、Windows 优化。

- **磁盘清理**：扫描常见缓存/临时/日志目录，展示可清理项及大小，支持勾选后删除，并可将
  某路径加入排除列表（持久化）
- **macOS / Windows 优化**：平台专用一键清理（macOS 的 Caches / .DS_Store，Windows 的 Temp）
- **磁盘占用概览**：罗列各挂载点/分区的已用、可用、总量

## 2. 后端结构

```
src-tauri/src/commands/tuning.rs
  scan_caches        — 扫描可清理目录，返回 Vec<CleanCandidate>
  clean_paths        — 删除指定路径，返回释放字节数（带安全校验）
  get_disk_usage     — 列出磁盘分区占用（Linux/macOS 用 df，Windows 用 wmic）
  list_exclusions / add_exclusion / remove_exclusion — 排除项持久化
  optimize_disk      — 平台专用一键优化
  clean_requires_sudo— 判断所选路径是否需要管理员权限
```

### 命令与数据结构

```rust
pub struct CleanCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub file_count: u64,
    pub is_dir: bool,
}
pub struct DiskUsage {
    pub mount: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub format: String,
}
```

- **扫描定位**：按平台返回若干预置候选目录（Linux: `/var/cache/apt`、`~/.cache`、`/tmp`、
  `/var/log`；macOS: `~/Library/Caches`、`/tmp`、`/var/log` 等；Windows: `%TEMP%`、
  `%LOCALAPPDATA%`），通过 `dir_size` 递归统计字节数与文件数
- **安全校验**（`is_cleanable`）：禁止删除根目录、家目录、`/usr` `/bin` `/etc` / `C:\Windows`
  等系统关键路径
- **排除项**持久化到 `data_dir/tune_exclusions.json`，扫描时自动跳过
- **一键优化**（`optimize_disk`）：Linux 调 `apt-get clean` / `pacman -Sc` / `dnf clean all`；
  macOS 扫 `~/Library/Caches` 与 `.DS_Store`；Windows 统计 `%TEMP%`

## 3. 前端结构

- `src/views/SystemTuning.vue` — 单页面，Tabs 由路由驱动（`/system-tune` → 磁盘清理、
  `/system-tune/mac` → macOS、`/system-tune/win` → Windows）
- `src/lib/nav-config.js` — `system-tune` 上下文子导航（磁盘/mac/win）
- `src/lib/icon-map.js` / `AppIcon.vue` — 新增 `apple` / `monitor` / `shield` 图标

## 4. 安全边界

- 删除前有 `is_cleanable` 白名单/黑名单双重校验，杜绝误删系统关键目录
- 系统级缓存清理（Linux）通常需要管理员权限，前端通过 `clean_paths` 返回值与
  `clean_requires_sudo` 提示用户
- 排除列表允许用户永久保护某些路径，避免重复误删