use crate::utils;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// 静态软件数据（下载 URL 表 / GUI 应用名单 / 软件定义表 / 包名映射）已拆出到 software_data 模块
#[path = "software_data.rs"]
mod software_data;
use software_data::{build_software_defs, get_download_url, GUI_APPS};

/// 包管理器安装/卸载执行体已拆出到 software_pm 模块
#[path = "software_pm.rs"]
pub(crate) mod software_pm;

/// 安装软件（跨平台，多包管理器支持）——执行体在 software_pm::install_software_exec
#[tauri::command]
pub async fn install_software(package_name: String) -> Result<String, String> {
    software_pm::install_software_exec(package_name).await
}

/// 卸载软件（跨平台，多包管理器支持）——执行体在 software_pm::uninstall_software_exec
#[tauri::command]
pub async fn uninstall_software(package_name: String) -> Result<String, String> {
    software_pm::uninstall_software_exec(package_name).await
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Software {
    pub name: String,
    pub category: String,
    pub version: String,
    pub status: String,
    pub action: String,
    pub package_name: Option<String>,
    pub available_versions: Vec<String>,
    pub download_supported: bool,
}

/// 安全获取软件版本：对 GUI 应用跳过，避免启动它们
/// 仅在真正超时时返回 "timeout"；其余失败返回具体错误，避免误报
async fn safe_get_version(cmd: &str) -> String {
    if GUI_APPS.contains(&cmd) {
        return "installed".to_string();
    }

    let cmd_str = cmd.to_string();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::task::spawn_blocking(move || {
            std::process::Command::new(&cmd_str)
                .arg("--version")
                .output()
        }),
    )
    .await;

    match result {
        Ok(Ok(Ok(output))) if output.status.success() => {
            let ver = String::from_utf8_lossy(&output.stdout);
            let first_line = ver.lines().next().unwrap_or("unknown");
            if first_line.len() > 60 {
                first_line[..57].to_string() + "..."
            } else {
                first_line.to_string()
            }
        }
        Ok(Ok(Ok(output))) => {
            let code = output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string());
            format!("version check failed (exit {})", code)
        }
        Ok(Ok(Err(e))) => format!("version check failed: {}", e),
        Ok(Err(e)) => format!("version check failed: {}", e),
        Err(_) => "timeout".to_string(),
    }
}

#[tauri::command]
pub async fn list_software() -> Vec<Software> {
    let defs = build_software_defs();
    let mut handles = Vec::with_capacity(defs.len());

    for s in &defs {
        let name = s.name;
        let cmd = s.cmd;
        let category = s.category;
        let pkg = s.package_name;
        handles.push(tokio::spawn(async move {
            let found = utils::find_cmd_path(cmd).is_some();
            let version = if found {
                safe_get_version(cmd).await
            } else {
                "N/A".to_string()
            };
            let status = if found { "installed" } else { "available" };
            let action = if found { "Uninstall" } else { "Install" };
            let download_url = get_download_url(name, "0.0.0");
            Software {
                name: name.to_string(),
                category: category.to_string(),
                version,
                status: status.to_string(),
                action: action.to_string(),
                package_name: Some(pkg.to_string()),
                available_versions: Vec::new(),
                download_supported: download_url.is_some(),
            }
        }));
    }

    let mut list = Vec::with_capacity(defs.len());
    for h in handles {
        if let Ok(sw) = h.await {
            list.push(sw);
        }
    }
    list
}

// ==================== 包管理器检测与包名映射 ====================

#[derive(Serialize, Clone)]
pub struct PackageManagerInfo {
    pub name: String,
    pub binary: String,
    pub needs_sudo: bool,
}

/// 列出系统上检测到的可用包管理器（前端用于展示引导提示）
#[tauri::command]
pub fn list_package_managers() -> Vec<PackageManagerInfo> {
    let managers = software_pm::detect_package_managers();
    managers
        .into_iter()
        .map(|pm| PackageManagerInfo {
            name: pm.name.to_string(),
            binary: pm.binary.to_string(),
            needs_sudo: pm.needs_sudo,
        })
        .collect()
}

/// 深度卸载：先执行标准卸载，再清理残留的配置文件、缓存和数据目录
#[tauri::command]
pub async fn uninstall_software_deep(
    package_name: String,
    app_name: String,
) -> Result<String, String> {
    // (a) 先执行标准卸载
    let result = software_pm::uninstall_software_exec(package_name.clone()).await;

    // 安全校验：只有标准卸载成功后才清理残留目录。
    // 若卸载失败（如包管理器报错、无权限），跳过目录删除，避免误删仍在使用的活动数据。
    let uninstall_ok = result.is_ok();
    if !uninstall_ok {
        return result;
    }

    // (b) 获取所有可能的清理路径（复用 residue_scanner::known_paths 单一数据源）
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let cleanup_paths =
        crate::residue_scanner::known_paths::get_cleanup_paths(&app_name, &package_name, &home);

    // (c) 遍历删除所有存在的目录
    let mut cleaned_dirs: Vec<String> = Vec::new();
    let mut error_dirs: Vec<String> = Vec::new();

    for path in &cleanup_paths {
        if path.exists() {
            match std::fs::remove_dir_all(path) {
                Ok(()) => {
                    cleaned_dirs.push(path.display().to_string());
                }
                Err(e) => {
                    error_dirs.push(format!("{} ({})", path.display(), e));
                }
            }
        }
    }

    // (d) 构造结果消息（至此卸载已成功）
    let mut message = result.clone().unwrap_or_default();
    if !cleaned_dirs.is_empty() || !error_dirs.is_empty() {
        message.push_str("\n\n");
    }
    if !cleaned_dirs.is_empty() {
        message.push_str(&format!("已清理目录:\n{}", cleaned_dirs.join("\n")));
    }
    if !error_dirs.is_empty() {
        if !cleaned_dirs.is_empty() {
            message.push('\n');
        }
        message.push_str(&format!("清理失败:\n{}", error_dirs.join("\n")));
    }

    Ok(message)
}

/// 已安装的系统应用（用于应用卸载管理器）
#[derive(Serialize, Clone)]
pub struct InstalledApp {
    pub name: String,
    pub version: String,
    pub source: String,       // 包管理器名称
    pub icon: Option<String>, // base64 data URL，无图标时为 None
}

/// 获取包管理器的"列出已安装"命令参数
fn get_pm_list_args(pm_name: &str) -> Option<&'static [&'static str]> {
    match pm_name {
        "apt" => Some(&["list", "--installed"]),
        "dnf" => Some(&["list", "installed"]),
        "pacman" => Some(&["-Qe"]),
        "zypper" => Some(&["se", "--installed-only"]),
        "apk" => Some(&["list", "--installed"]),
        "Homebrew" => Some(&["list", "--formula", "--versions"]),
        "MacPorts" => Some(&["installed"]),
        "winget" => Some(&["list", "--accept-source-agreements"]),
        "chocolatey" => Some(&["list", "--local-only"]),
        "snap" => Some(&["list"]),
        "flatpak" => Some(&[
            "list",
            "--app",
            "--columns=application,version,branch,origin",
        ]),
        _ => None,
    }
}

/// 解析包管理器列表命令的输出
fn parse_pm_list_output(pm_name: &str, stdout: &str) -> Vec<(String, String)> {
    let mut apps = Vec::new();
    match pm_name {
        "apt" => {
            // apt list --installed 输出: pkgname/stable,now VERSION arch [installed]
            // 只保留用户手动安装的包（[installed]），跳过 [installed,automatic]
            // 依赖标记——否则系统库/依赖包（2183 个）全被当成"软件"列出，
            // 这是用户看到卸载列表混入大量依赖的根因。
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty() || !line.contains("[installed") || line.contains("automatic") {
                    continue;
                }
                let pkg = line.split('/').next().unwrap_or("").trim();
                if pkg.is_empty() {
                    continue;
                }
                // 提取版本：在 ",now " 之后到第一个空格
                let version = if let Some(pos) = line.find(",now ") {
                    let after = &line[pos + 5..];
                    after.split_whitespace().next().unwrap_or("unknown")
                } else if let Some(pos) = line.find("now ") {
                    let after = &line[pos + 4..];
                    after.split_whitespace().next().unwrap_or("unknown")
                } else {
                    "unknown"
                };
                apps.push((pkg.to_string(), version.to_string()));
            }
        }
        "pacman" => {
            // pacman -Q: "pkg VERSION"
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && !parts[0].starts_with("local/") {
                    apps.push((parts[0].to_string(), parts[1].to_string()));
                } else if parts.len() == 2 {
                    apps.push((
                        parts[0].trim_start_matches("local/").to_string(),
                        parts[1].to_string(),
                    ));
                }
            }
        }
        "apk" => {
            // apk list --installed: "pkgname-version arch {pkgname} ... [installed]"
            // 只保留手动安装（[installed]），跳过 [installed,automatic] 依赖包
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty() || !line.contains("[installed") || line.contains("automatic") {
                    continue;
                }
                let first = line.split_whitespace().next().unwrap_or("");
                // apk 格式: pkgname-version → 从右向左找第一个版本开始的位置
                let mut split_pos = None;
                for (i, c) in first.char_indices().rev() {
                    if c == '-' && i > 0 {
                        let prev = first.as_bytes()[i - 1];
                        if prev.is_ascii_digit() || prev == b'.' {
                            split_pos = Some(i);
                            break;
                        }
                    }
                }
                if let Some(pos) = split_pos {
                    apps.push((first[..pos].to_string(), first[pos + 1..].to_string()));
                } else {
                    apps.push((first.to_string(), "installed".to_string()));
                }
            }
        }
        "dnf" => {
            // dnf list installed: "pkg.arch VERSION @repo"
            let mut in_data = false;
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line.contains("Installed Packages") || line.contains("Available Packages") {
                    in_data = line.contains("Installed Packages");
                    continue;
                }
                if !in_data {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[0].contains('.') {
                    let pkg = parts[0]
                        .rsplit_once('.')
                        .map(|(n, _)| n)
                        .unwrap_or(parts[0]);
                    apps.push((pkg.to_string(), parts[1].to_string()));
                }
            }
        }
        "zypper" => {
            // zypper se --installed-only: "i | pkg | summary | version | arch | repo"
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty() || !line.starts_with('i') {
                    continue;
                }
                let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
                // zypper 状态列：i = 自动安装（依赖），i+ = 手动安装
                // 只保留 i+（用户主动安装的软件），排除依赖包
                if parts.len() >= 4 && parts[0] == "i+" {
                    apps.push((parts[1].to_string(), parts[3].to_string()));
                }
            }
        }
        "Homebrew" => {
            // brew list --formula --versions: "pkg VERSION"
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let mut parts = line.split_whitespace();
                if let Some(pkg) = parts.next() {
                    let ver = parts.collect::<Vec<&str>>().join(" ");
                    if !ver.is_empty() {
                        apps.push((pkg.to_string(), ver));
                    }
                }
            }
        }
        "MacPorts" => {
            // port installed: "  pkg @version"
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty()
                    || line.starts_with("The following ports are currently installed")
                {
                    continue;
                }
                if let Some(at_pos) = line.find(" @") {
                    let pkg = line[..at_pos].trim();
                    let ver = line[at_pos + 2..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("unknown");
                    if !pkg.is_empty() {
                        apps.push((pkg.to_string(), ver.to_string()));
                    }
                }
            }
        }
        "winget" => {
            // winget list 输出: 表头后 "Name  Id  Version  Available  Source"
            let mut found_header = false;
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if !found_header {
                    if line.contains("Name") && line.contains("Id") && line.contains("Version") {
                        found_header = true;
                    }
                    continue;
                }
                if line.contains("---") || line.contains("──") {
                    continue;
                }
                // 按制表符或多个空格拆分
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    apps.push((parts[0].to_string(), parts[2].to_string()));
                }
            }
        }
        "chocolatey" => {
            // choco list --local-only: "pkg VERSION"
            for line in stdout.lines() {
                let line = line.trim();
                if line.is_empty()
                    || line.starts_with("Chocolatey")
                    || line.contains("packages installed")
                {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    apps.push((parts[0].to_string(), parts[1].to_string()));
                }
            }
        }
        "snap" => {
            // snap list: "Name  Version  Rev  Tracking  Publisher  Notes"
            // Skip header line
            for (i, line) in stdout.lines().enumerate() {
                if i == 0 {
                    continue;
                } // skip header
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    apps.push((parts[0].to_string(), parts[1].to_string()));
                }
            }
        }
        "flatpak" => {
            // flatpak list --app --columns=application,version,branch,origin
            // "ApplicationID  Version  Branch  Origin"
            // Skip header line
            for (i, line) in stdout.lines().enumerate() {
                if i == 0 {
                    continue;
                } // skip header
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    apps.push((parts[0].to_string(), parts[1].to_string()));
                } else if parts.len() == 1 {
                    apps.push((parts[0].to_string(), "installed".to_string()));
                }
            }
        }
        _ => {}
    }
    apps
}

/// 列出系统上所有已安装应用（跨平台，多包管理器支持）
/// 用于"应用卸载管理器"模块
#[tauri::command]
pub async fn list_installed_apps() -> Result<Vec<InstalledApp>, String> {
    let mut all_apps: Vec<InstalledApp> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // (1) 主来源：GUI 应用（.desktop 文件）。Windows/macOS 上退化为包管理器列表。
    #[cfg(target_os = "linux")]
    {
        for app in list_gui_apps() {
            let key = format!("{}:{}", app.name, app.source);
            if seen.insert(key) {
                all_apps.push(app);
            }
        }
    }

    // (2) 包管理器列表：只补充包管理器中未覆盖的 GUI 应用（通过 desktop 匹配判断）
    let managers = software_pm::detect_package_managers();
    for pm in &managers {
        let Some(args) = get_pm_list_args(pm.name) else {
            continue;
        };

        let output = Command::new(pm.binary)
            .args(args)
            .env("LANG", "C")
            .output()
            .map_err(|e| format!("Failed to execute {}: {}", pm.binary, e))?;

        if !output.status.success() {
            continue; // 跳过失败的 PM
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let apps = parse_pm_list_output(pm.name, &stdout);

        for (name, version) in apps {
            // snap 运行时/基底包不是软件（core20/core22/snapd/bare/主题包等）
            if pm.name == "snap" && is_snap_runtime_package(&name) {
                continue;
            }

            // Linux：包名与 desktop 主来源匹配时，不新增重复条目，
            // 而是把真实版本号合并到已有条目（desktop 版本常为 "installed"）。
            // 否则同一软件会出现两条：一条有版本号（包管理器）、一条 "installed"（desktop）。
            #[cfg(target_os = "linux")]
            {
                if let Some(display_name) = desktop_display_name_for_package(&name) {
                    // 先用 desktop 显示名精确匹配（如 "Visual Studio Code"），
                    // 再回退到归一化名匹配（如 "reasonix-desktop" ↔ "Reasonix"）
                    let mut matched = false;
                    for app in all_apps.iter_mut() {
                        if app.name == display_name {
                            merge_versions(&mut app.version, &version);
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        let pkg_norm = normalize_app_name(&name);
                        for app in all_apps.iter_mut() {
                            if normalize_app_name(&app.name) == pkg_norm {
                                merge_versions(&mut app.version, &version);
                                matched = true;
                                break;
                            }
                        }
                    }
                    if !matched {
                        // desktop 匹配到但条目未加入（不应发生），按 GUI 判定新增
                        let icon = resolve_app_icon(&name, pm.name);
                        if icon.is_some() {
                            let key = format!("{}:{}", name, pm.name);
                            if seen.insert(key) {
                                all_apps.push(InstalledApp {
                                    name,
                                    version,
                                    source: pm.name.to_string(),
                                    icon,
                                });
                            }
                        }
                    }
                    continue;
                }
                // 无 desktop 入口 → 需有图标才保留（GUI 判定），否则视为 CLI/系统组件
                let icon = resolve_app_icon(&name, pm.name);
                if icon.is_none() {
                    continue;
                }
                let key = format!("{}:{}", name, pm.name);
                if seen.insert(key) {
                    all_apps.push(InstalledApp {
                        name,
                        version,
                        source: pm.name.to_string(),
                        icon,
                    });
                }
                continue;
            }

            // Windows/macOS：无 .desktop 概念，包管理器条目全部保留，
            // 否则列表会被清空（图标与 desktop 判定恒为空）。
            #[cfg(not(target_os = "linux"))]
            {
                let icon = resolve_app_icon(&name, pm.name);
                let key = format!("{}:{}", name, pm.name);
                if seen.insert(key) {
                    all_apps.push(InstalledApp {
                        name,
                        version,
                        source: pm.name.to_string(),
                        icon,
                    });
                }
            }
        }
    }

    // 按名称排序
    all_apps.sort_by_key(|a| a.name.to_lowercase());

    Ok(all_apps)
}

#[cfg(target_os = "linux")]
fn is_system_desktop(path: &std::path::Path) -> bool {
    // 排除 GNOME/系统组件桌面入口
    let s = path.to_string_lossy().to_lowercase();
    let name = path
        .file_stem()
        .map(|x| x.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    s.contains("/system-applications/")
        || s.contains("/autostart/")
        || name.starts_with("gnome-")
        || name.starts_with("org.gnome.")
        || name.ends_with("-autostart")
        || name == "org.gnome.software"
        || name.contains("tracker")
        || name.contains("zeitgeist")
        || name.contains("bluetooth-sendto")
        || name == "apport-gtk"
        || name == "gnome-software"
}

/// 扫描系统 .desktop 文件，构建 GUI 应用列表
#[cfg(target_os = "linux")]
fn list_gui_apps() -> Vec<InstalledApp> {
    use std::collections::HashMap;

    let desktop_dirs = [
        dirs_home_dir().join(".local/share/applications"),
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];

    let mut apps: Vec<InstalledApp> = Vec::new();
    // 归一化显示名 -> 索引。同软件多个 desktop 文件（如 "CC Switch.desktop" 与
    // "cc-switch.desktop"）归一化后相同，只保留一条；多来源时合并版本。
    let mut seen: HashMap<String, usize> = HashMap::new();

    for (dir_idx, dir) in desktop_dirs.iter().enumerate() {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e != "desktop").unwrap_or(true) {
                continue;
            }
            if is_system_desktop(&path) {
                continue;
            }
            let file_stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if file_stem.is_empty() || file_stem.ends_with(".desktop") {
                continue; // 跳过如 "ai.opencode.desktop.desktop" 的嵌套
            }
            let source = match dir_idx {
                0 => "manual", // ~/.local/share/applications
                1 => "system", // /usr/share/applications
                2 => "flatpak",
                _ => "snap",
            };
            // 识别实际包管理器来源（通过 desktop 文件的 X-Flatpak/或路径）
            let path_str = path.to_string_lossy().to_string();
            let actual_source = if path_str.contains("/flatpak/") {
                "flatpak"
            } else if path_str.contains("/snapd/") {
                "snap"
            } else {
                source
            };

            let name = desktop_display_name(&path);
            // 用显示名归一化去重；空显示名回退到文件 stem
            let dedup_key = if name.is_empty() {
                normalize_app_name(&file_stem)
            } else {
                normalize_app_name(&name)
            };
            if dedup_key.is_empty() {
                continue;
            }

            if let Some(&idx) = seen.get(&dedup_key) {
                // 同一软件已存在：合并版本，优先保留非 "system" 来源（用户安装优先）
                let existing = &mut apps[idx];
                merge_versions(&mut existing.version, &desktop_version(&path));
                // manual/flatpak/snap 优先级高于 system；已有条目是 system 时升级来源
                if existing.source == "system" && actual_source != "system" {
                    existing.source = actual_source.to_string();
                }
                if existing.icon.is_none() {
                    existing.icon = read_desktop_icon(&path);
                }
                continue;
            }

            let version = desktop_version(&path);
            let icon = read_desktop_icon(&path);
            seen.insert(dedup_key, apps.len());
            apps.push(InstalledApp {
                name,
                version,
                source: actual_source.to_string(),
                icon,
            });
        }
    }

    apps
}

/// 读取 .desktop 文件的显示名称（Name=，排除本地化 Name[xx]=）
#[cfg(target_os = "linux")]
fn desktop_display_name(path: &std::path::Path) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        return path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
    };
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("Name=") && !line.starts_with("Name[") {
            let name = line[5..].trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// 读取 .desktop 文件的版本字段。
/// 优先级：X-AppVersion= / Version= → 从 Exec= 可执行路径中提取版本目录
/// （如 JetBrains toolbox: ~/.../apps/clion-2026.1.1/bin/clion.sh → 2026.1.1）
/// → 从 Name= 显示名尾部提取（如 "CLion 2026.1.1" → 2026.1.1）。
/// 只做静态解析，绝不执行任何命令，避免启动 GUI 应用。
#[cfg(target_os = "linux")]
fn desktop_version(path: &std::path::Path) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        return "installed".to_string();
    };
    let mut exec_line: Option<String> = None;
    let mut name_line: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line
            .strip_prefix("X-AppVersion=")
            .or_else(|| line.strip_prefix("Version="))
        {
            let v = v.trim();
            if !v.is_empty() && v != "1.0" {
                return v.to_string();
            }
        }
        if let Some(v) = line.strip_prefix("Exec=") {
            exec_line = Some(v.trim().to_string());
        }
        if let Some(v) = line.strip_prefix("Name=") {
            if !line.starts_with("Name[") {
                name_line = Some(v.trim().to_string());
            }
        }
    }
    // 从 Exec 路径中的版本目录提取版本号（如 "clion-2026.1.1"、"pycharm-2026.2"）
    if let Some(exec) = exec_line {
        for seg in exec.split(['/', '\\']) {
            let seg = seg.trim();
            if seg.is_empty() {
                continue;
            }
            // 匹配 "<name>-<version>" 形态，版本以数字开头
            if let Some((name, ver)) = seg.rsplit_once('-') {
                if ver.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && ver
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
                    && ver.len() <= 24
                    && !name.is_empty()
                {
                    return ver.to_string();
                }
            }
        }
    }
    // 从显示名尾部提取版本（"CLion 2026.1.1"、"Android Studio Quail 2 2026.1.2"）
    if let Some(name) = name_line {
        let trimmed = name.trim();
        if let Some(ver) = trimmed.split_whitespace().last() {
            if ver.chars().next().is_some_and(|c| c.is_ascii_digit())
                && ver
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
                && ver.len() <= 24
            {
                return ver.to_string();
            }
        }
    }
    "installed".to_string()
}

/// 读取 .desktop 文件的图标（复用桌面图标解析逻辑）
#[cfg(target_os = "linux")]
fn read_desktop_icon(path: &std::path::Path) -> Option<String> {
    desktop_icon_path(path).and_then(|p| read_image_as_data_url(&p))
}

/// snap 的运行时/基底/主题包不是软件，应排除：
/// core20/core22/core24（运行时基底）、snapd（快照守护进程）、
/// bare（空基底）、gtk-common-themes/snapd-desktop-integration（主题/集成）
fn is_snap_runtime_package(name: &str) -> bool {
    let n = name.to_lowercase();
    n == "snapd"
        || n == "bare"
        || n == "core"
        || n.starts_with("core1")
        || n.starts_with("core2")
        || n == "gtk-common-themes"
        || n == "snapd-desktop-integration"
        || n == "snap-store"
        || n.ends_with("-theme")
}

/// 归一化应用名用于去重匹配：
/// 小写 + 去掉所有非字母数字字符（- _ 空格 . 等），
/// 使 "steam-launcher" / "Steam" / "steam_launcher" 都归一为 "steamlauncher"。
/// 同时去掉常见的包名后缀（-launcher/-desktop/-bin/-client/-player/-studio/-common/-libs），
/// 使 "reasonix-desktop" 与 "Reasonix"、"steam-libs-amd64" 与 "Steam" 能正确配对。
/// 仅 Linux 使用（调用方均在 cfg(target_os="linux") 内），未加门控会在 Windows/macOS 上产生 dead_code 警告。
#[cfg(target_os = "linux")]
fn normalize_app_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut base = lower
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>();
    // 去掉 libs 后缀族（amd64/i386 架构变体）
    let stripped = [
        "launcher", "desktop", "client", "player", "studio", "common", "bin",
    ];
    for suffix in stripped {
        if base.ends_with(suffix) && base.len() > suffix.len() + 2 {
            base.truncate(base.len() - suffix.len());
            break;
        }
    }
    // libs-amd64 / libs-i386 等：去掉 "libs" 及架构尾巴
    if base.ends_with("libs") && base.len() > 6 {
        base.truncate(base.len() - 4);
    }
    base
}

/// 判断包名是否有对应 desktop 入口（用于包管理器列表去重 GUI 应用）。
/// 归一化匹配：desktop 文件名 / Name= 归一化后与包名归一化后一致即命中。
/// 相比旧代码：精确匹配会漏掉 "steam-launcher"↔"Steam" 这类后缀差异；
/// contains 子串匹配会误命中 "code"↔"libcodec"。
/// 归一化（去分隔符+去后缀）在两者之间取得平衡。
#[cfg(target_os = "linux")]
fn desktop_display_name_for_package(package: &str) -> Option<String> {
    let pkg_norm = normalize_app_name(package);
    if pkg_norm.is_empty() {
        return None;
    }
    // 显式别名表：包名(归一化) → 软件显示名。覆盖名称完全不同的已知映射，
    // 如 apt 包 "code"(VSCode) 与显示名 "Visual Studio Code"。
    // 不依赖启发式弱匹配，避免 "zcode"↔"code" 这类误匹配。
    let aliases: &[(&str, &str)] = &[
        ("code", "Visual Studio Code"),
        ("codium", "VSCodium"),
        ("codeoss", "Visual Studio Code - OSS"),
        ("wps", "WPS Office"),
    ];
    if let Some((_, display)) = aliases.iter().find(|(a, _)| *a == pkg_norm) {
        return Some((*display).to_string());
    }

    let desktop_dirs = [
        dirs_home_dir().join(".local/share/applications"),
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];

    // desktop 文件名精确匹配（code.desktop ↔ 包 "code"）。
    // 优先返回文件名命中的条目，避免 url-handler 等辅助入口抢先。
    for dir in &desktop_dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e != "desktop").unwrap_or(true) {
                continue;
            }
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if normalize_app_name(&stem) == pkg_norm {
                let display = desktop_display_name(&path);
                if !display.is_empty() {
                    return Some(display);
                }
                return Some(stem);
            }
        }
    }

    // desktop 显示名（Name=）精确匹配
    for dir in &desktop_dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e != "desktop").unwrap_or(true) {
                continue;
            }
            let name = desktop_name_of(&path);
            if !name.is_empty() && normalize_app_name(&name) == pkg_norm {
                return Some(name);
            }
        }
    }
    None
}

/// 把新版本合并进已有版本字符串（多来源去重，如 desktop 的 "installed" 与 apt 的 "1.24.1"）。
/// 规则：
///   - 已有值为 "installed"/"unknown"/"N/A" 且新值真实 -> 直接替换
///   - 新值与已有相同 -> 不变
///   - 否则以 "a, b" 合并（同一软件不同来源/多版本共存）
///
/// 仅 Linux 使用（调用方均在 cfg(target_os="linux") 内），未加门控会在 Windows/macOS 上产生 dead_code 警告。
#[cfg(target_os = "linux")]
fn merge_versions(existing: &mut String, new_version: &str) {
    let cur = existing.trim();
    let new_ver = new_version.trim();
    if cur.is_empty() {
        *existing = new_ver.to_string();
        return;
    }
    let placeholder = ["installed", "unknown", "N/A"];
    let cur_is_placeholder = placeholder.contains(&cur);
    let new_is_placeholder = placeholder.contains(&new_ver);

    if cur == new_ver || new_is_placeholder {
        return; // 无变化
    }
    if cur_is_placeholder {
        *existing = new_ver.to_string();
        return;
    }
    // 都真实且不同 → 合并（去重）
    if !cur.split(',').any(|v| v.trim() == new_ver) {
        existing.push_str(&format!(", {}", new_ver));
    }
}

/// 从 Linux 桌面入口文件（.desktop）解析应用图标，返回 base64 data URL
fn resolve_app_icon(app_name: &str, _source: &str) -> Option<String> {
    // 只在 Linux 上解析；Windows/macOS 暂不处理（由前端品牌图标兜底）
    #[cfg(target_os = "linux")]
    {
        #[allow(unused_imports)]
        use base64::Engine as _;
        use std::path::Path;

        let desktop_dirs = [
            dirs_home_dir().join(".local/share/applications"),
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/var/lib/flatpak/exports/share/applications"),
            PathBuf::from("/var/lib/snapd/desktop/applications"),
        ];

        // 1. 构造候选 .desktop 文件名
        //    flatpak/snap: app id 即文件名（如 org.example.App.desktop）
        //    apt: 尝试按包名、按 Desktop 文件里 Name= 匹配
        let mut candidates: Vec<PathBuf> = Vec::new();
        for dir in &desktop_dirs {
            if !dir.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(dir).ok()? {
                let Ok(entry) = entry else { continue };
                let path = entry.path();
                let Some(ext) = path.extension() else {
                    continue;
                };
                if ext != "desktop" {
                    continue;
                }
                let file_stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let desktop_name = desktop_name_of(&path);
                // 精确匹配：文件名 或 desktop 的 Name=/Name[xx]= 与包名一致
                if file_stem == app_name.to_lowercase()
                    || desktop_name.to_lowercase() == app_name.to_lowercase()
                    || desktop_name.contains(app_name)
                    || app_name.to_lowercase().contains(&file_stem)
                {
                    candidates.push(path);
                    if file_stem == app_name.to_lowercase() {
                        break;
                    }
                }
            }
            if !candidates.is_empty() {
                break;
            }
        }

        for desktop in candidates {
            if let Some(icon_path) = desktop_icon_path(&desktop) {
                if let Some(data) = read_image_as_data_url(&icon_path) {
                    return Some(data);
                }
            }
        }

        // 2. 兜底：直接在图标主题目录按包名搜索（如 apt 的 flatpak 图标等）
        let theme_dirs = [
            "/usr/share/icons/hicolor",
            "/usr/share/icons/Adwaita",
            "/usr/share/icons/ubuntu-mono-dark",
            "/usr/share/pixmaps",
        ];
        for theme in theme_dirs {
            for size in [
                "512x512", "256x256", "128x128", "64x64", "48x48", "32x32", "scalable",
            ] {
                let dir = Path::new(theme).join(size).join("apps");
                if !dir.is_dir() {
                    continue;
                }
                for ext in ["png", "svg", "xpm", "svgz"] {
                    let p = dir.join(format!("{}.{}", app_name, ext));
                    if p.is_file() {
                        if let Some(data) = read_image_as_data_url(&p) {
                            return Some(data);
                        }
                    }
                }
            }
        }

        None
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app_name, _source);
        None
    }
}

#[cfg(target_os = "linux")]
fn dirs_home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/root"))
}

/// 读取 .desktop 文件的 Name 字段（首个非注释）
#[cfg(target_os = "linux")]
fn desktop_name_of(path: &std::path::Path) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        return String::new();
    };
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("Name=") && !line.starts_with("Name[") {
            return line[5..].trim().to_string();
        }
    }
    String::new()
}

/// 从 .desktop 文件解析 Icon= 字段，返回可读取的图标路径
#[cfg(target_os = "linux")]
fn desktop_icon_path(desktop: &std::path::Path) -> Option<PathBuf> {
    use std::path::Path;

    let content = std::fs::read_to_string(desktop).ok()?;
    let mut icon_value: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("Icon=") {
            icon_value = Some(v.trim().to_string());
            break;
        }
    }
    let icon = icon_value?;
    if icon.is_empty() {
        return None;
    }
    let icon_path = Path::new(&icon);
    // 绝对路径
    if icon_path.is_absolute() && icon_path.is_file() {
        return Some(icon_path.to_path_buf());
    }
    // 相对路径（如 hicolor/...）
    if icon.contains('/') && !icon.starts_with('/') {
        for base in ["/usr/share", "/usr/local/share"] {
            let p = Path::new(base).join(&icon);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // 图标名：在图标主题目录中搜索（无扩展名则尝试常见扩展）
    let name_without_ext = icon
        .rsplit_once('.')
        .map(|(n, e)| {
            if matches!(e, "png" | "svg" | "svgz" | "xpm") {
                n.to_string()
            } else {
                icon.clone()
            }
        })
        .unwrap_or_else(|| icon.clone());

    let theme_dirs = [
        "/usr/share/icons/hicolor",
        "/usr/share/icons/Adwaita",
        "/usr/share/icons/ubuntu-mono-dark",
        "/usr/share/pixmaps",
        "/usr/share/icons/breeze",
        "/usr/share/icons/hicolor",
    ];
    for theme in theme_dirs {
        if let Some(icon_name_only) = name_without_ext.rsplit('/').next() {
            for size in [
                "512x512", "256x256", "128x128", "96x96", "64x64", "48x48", "32x32", "24x24",
                "scalable",
            ] {
                for ext in ["png", "svg", "svgz", "xpm"] {
                    let p = Path::new(theme)
                        .join(size)
                        .join("apps")
                        .join(format!("{}.{}", icon_name_only, ext));
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
    }
    // 直接尝试 pixmaps 目录
    let pixmap = Path::new("/usr/share/pixmaps").join(&icon);
    if pixmap.is_file() {
        return Some(pixmap);
    }
    let pixmap_ext = Path::new("/usr/share/pixmaps").join(format!("{}.png", icon));
    if pixmap_ext.is_file() {
        return Some(pixmap_ext);
    }
    None
}

/// 读取图片文件并转为 base64 data URL
/// L2 修复：单图标上限从 2MB 收紧到 512KB，避免大量应用列表时
/// 同步读取 + base64 化导致 UI 卡顿与 IPC 负载过大。
#[cfg(target_os = "linux")]
fn read_image_as_data_url(path: &std::path::Path) -> Option<String> {
    use base64::Engine as _;
    let data = std::fs::read(path).ok()?;
    if data.is_empty() || data.len() > 512 * 1024 {
        return None; // 空文件或超大文件跳过
    }
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("svg") | Some("svgz") => "image/svg+xml",
        Some("png") => "image/png",
        Some("xpm") => "image/x-xpm",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        _ => return None,
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Some(format!("data:{};base64,{}", mime, b64))
}

/// 从 GitHub Releases API 获取版本列表
async fn fetch_github_versions(owner: &str, repo: &str) -> Result<Vec<String>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=30",
        owner, repo
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let resp = client
        .get(&url)
        .header("User-Agent", "DevNexus/1.0")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch versions: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API returned {}", resp.status()));
    }
    let releases: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    let versions: Vec<String> = releases
        .iter()
        .filter_map(|r| r.get("tag_name").and_then(|v| v.as_str()))
        .filter(|v| {
            !v.contains("rc")
                && !v.contains("beta")
                && !v.contains("alpha")
                && !v.contains("nightly")
        })
        .map(|v| v.trim_start_matches('v').to_string())
        .collect();
    if versions.is_empty() {
        Err("No stable releases found".to_string())
    } else {
        Ok(versions)
    }
}

/// 从 Node.js 官方 dist 目录获取版本列表
async fn fetch_node_versions() -> Result<Vec<String>, String> {
    let url = "https://nodejs.org/dist/index.json";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Node.js versions: {}", e))?;
    let versions: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    let result: Vec<String> = versions
        .iter()
        .filter_map(|v| v.get("version").and_then(|x| x.as_str()))
        .map(|v| v.trim_start_matches('v').to_string())
        .take(30)
        .collect();
    if result.is_empty() {
        Err("No Node.js versions found".to_string())
    } else {
        Ok(result)
    }
}

/// 从 Go 官方下载页获取版本列表
async fn fetch_go_versions() -> Result<Vec<String>, String> {
    let url = "https://go.dev/dl/?mode=json";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Go versions: {}", e))?;
    let versions: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    let result: Vec<String> = versions
        .iter()
        .filter_map(|v| v.get("version").and_then(|x| x.as_str()))
        .map(|v| v.trim_start_matches("go").to_string())
        .collect();
    if result.is_empty() {
        Err("No Go versions found".to_string())
    } else {
        Ok(result)
    }
}

/// 获取软件的可用版本列表（前端懒加载调用）
#[tauri::command]
pub async fn fetch_software_versions(package_name: String) -> Result<Vec<String>, String> {
    let defs = build_software_defs();
    let def = defs
        .iter()
        .find(|d| d.package_name == package_name || d.name == package_name)
        .ok_or_else(|| format!("Unknown software: {}", package_name))?;

    match def.name {
        "Node.js" => fetch_node_versions().await,
        "Go" => fetch_go_versions().await,
        name => {
            let (owner, repo) = match name {
                "Visual Studio Code" => ("microsoft", "vscode"),
                "Neovim" => ("neovim", "neovim"),
                "Git" => ("git", "git"),
                "Rust" => ("rust-lang", "rust"),
                "Python 3" => ("python", "cpython"),
                _ => return Err(format!("No version API configured for {}", name)),
            };
            fetch_github_versions(owner, repo).await
        }
    }
}

/// 获取当前平台的安装基目录
fn get_install_base_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    let base = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".local/share/devnexus/software")
    };
    #[cfg(target_os = "macos")]
    let base = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join("Library/Application Support/devnexus/software")
    };
    #[cfg(target_os = "windows")]
    let base = {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(appdata).join("devnexus/software")
    };
    std::fs::create_dir_all(&base).ok();
    base
}

/// 递归查找二进制文件（最多 5 层深度）
fn find_binary_in_dir(dir: &std::path::Path, name: &str) -> Option<PathBuf> {
    let exe_name = if cfg!(target_os = "windows") {
        format!("{}.exe", name)
    } else {
        name.to_string()
    };

    let mut dirs_to_check = vec![dir.to_path_buf()];
    let mut depth = 0;

    while !dirs_to_check.is_empty() && depth < 5 {
        let mut next_level = Vec::new();
        for current in dirs_to_check {
            if let Ok(entries) = std::fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        next_level.push(path);
                    } else if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                        if fname == exe_name || fname == name {
                            return Some(path);
                        }
                    }
                }
            }
        }
        dirs_to_check = next_level;
        depth += 1;
    }
    None
}

fn is_valid_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// 从官方源下载并安装指定版本的软件
#[tauri::command]
pub async fn install_software_from_url(
    package_name: String,
    version: String,
) -> Result<String, String> {
    if !is_valid_version(&version) {
        return Err(format!("Invalid version string: {}", version));
    }
    let defs = build_software_defs();
    let def = defs
        .iter()
        .find(|d| d.package_name == package_name || d.name == package_name)
        .ok_or_else(|| format!("Unknown software: {}", package_name))?;

    let url = get_download_url(def.name, &version)
        .ok_or_else(|| format!("No download URL configured for {}", def.name))?;

    let install_dir = get_install_base_dir().join(&package_name).join(&version);
    if install_dir.exists() {
        return Err(format!(
            "Version {} of {} is already installed at {}",
            version,
            def.name,
            install_dir.display()
        ));
    }

    let temp_dir = std::env::temp_dir().join("devnexus-install");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // 下载（M2 修复：连接/总超时 + 流式写入 + 大小上限，避免挂起与内存爆炸）
    const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB
    let filename = url.rsplit('/').next().unwrap_or("download");
    let filepath = temp_dir.join(filename);

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to download: {}", e))?;

    if let Some(total) = response.content_length() {
        if total > MAX_DOWNLOAD_BYTES {
            return Err(format!(
                "Download too large ({} bytes > {} limit)",
                total, MAX_DOWNLOAD_BYTES
            ));
        }
    }

    let mut out =
        std::fs::File::create(&filepath).map_err(|e| format!("Failed to create file: {}", e))?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Failed to read response: {}", e))?;
        downloaded += chunk.len() as u64;
        if downloaded > MAX_DOWNLOAD_BYTES {
            let _ = std::fs::remove_file(&filepath);
            return Err(format!(
                "Download exceeds size limit ({} bytes > {} limit)",
                downloaded, MAX_DOWNLOAD_BYTES
            ));
        }
        std::io::Write::write_all(&mut out, &chunk)
            .map_err(|e| format!("Failed to save file: {}", e))?;
    }

    // 创建安装目录
    std::fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Failed to create install dir: {}", e))?;

    // 解压
    let filename_lower = filename.to_lowercase();
    if filename_lower.ends_with(".tar.gz") || filename_lower.ends_with(".tgz") {
        let output = Command::new("tar")
            .args([
                "-xzf",
                &filepath.to_string_lossy(),
                "-C",
                &install_dir.to_string_lossy(),
            ])
            .output()
            .map_err(|e| format!("Failed to run tar: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "Extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    } else if filename_lower.ends_with(".tar.xz") {
        let output = Command::new("tar")
            .args([
                "-xJf",
                &filepath.to_string_lossy(),
                "-C",
                &install_dir.to_string_lossy(),
            ])
            .output()
            .map_err(|e| format!("Failed to run tar: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "Extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    } else if filename_lower.ends_with(".zip") {
        let output = Command::new("unzip")
            .args([
                "-o",
                &filepath.to_string_lossy(),
                "-d",
                &install_dir.to_string_lossy(),
            ])
            .output()
            .map_err(|e| format!("Failed to run unzip: {}", e))?;
        if !output.status.success() {
            // fallback: 用 Rust zip 库解压
            let file =
                std::fs::File::open(&filepath).map_err(|e| format!("Failed to open zip: {}", e))?;
            let mut archive =
                zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;
            for i in 0..archive.len() {
                let mut entry = archive
                    .by_index(i)
                    .map_err(|e| format!("Failed to read zip entry: {}", e))?;
                // 防止 zip-slip 路径穿越
                let entry_name = entry.name().replace('\\', "/");
                let sanitized = entry_name.split('/').fold(String::new(), |acc, part| {
                    if part == ".." {
                        // 忽略向上的路径遍历
                        acc
                    } else if part == "." || part.is_empty() {
                        acc
                    } else if acc.is_empty() {
                        part.to_string()
                    } else {
                        format!("{}/{}", acc, part)
                    }
                });
                let outpath = install_dir.join(&sanitized);
                // 确保路径没有逃逸出 install_dir
                if !outpath.starts_with(&install_dir) {
                    continue;
                }
                if entry.is_dir() {
                    std::fs::create_dir_all(&outpath).ok();
                } else {
                    if let Some(parent) = outpath.parent() {
                        std::fs::create_dir_all(parent).ok();
                    }
                    let mut outfile = std::fs::File::create(&outpath)
                        .map_err(|e| format!("Failed to create {}: {}", outpath.display(), e))?;
                    std::io::copy(&mut entry, &mut outfile)
                        .map_err(|e| format!("Failed to extract {}: {}", outpath.display(), e))?;
                }
            }
        }
    } else if filename_lower.ends_with(".dmg") {
        #[cfg(target_os = "macos")]
        {
            let mount_point = format!("/Volumes/{}", def.name);
            let _ = Command::new("hdiutil")
                .args(["attach", &filepath.to_string_lossy()])
                .output();
            let _ = Command::new("cp")
                .args([
                    "-R",
                    &format!("{}/{}", mount_point, def.name),
                    &install_dir.to_string_lossy(),
                ])
                .output();
            let _ = Command::new("hdiutil")
                .args(["detach", &mount_point])
                .output();
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Err("DMG files are only supported on macOS".to_string());
        }
    } else {
        // 可执行文件直接复制
        std::fs::copy(&filepath, install_dir.join(filename))
            .map_err(|e| format!("Failed to copy file: {}", e))?;
    }

    // 清理临时文件
    std::fs::remove_file(&filepath).ok();

    // 创建符号链接到 bin 目录
    let install_base = get_install_base_dir();
    let bin_dir = install_base.join("bin");
    std::fs::create_dir_all(&bin_dir).map_err(|e| format!("Failed to create bin dir: {}", e))?;

    let binary_name = match def.name {
        "Visual Studio Code" => "code",
        "Neovim" => "nvim",
        "Node.js" => "node",
        "Python 3" => "python3",
        _ => def.cmd,
    };

    if let Some(binary_path) = find_binary_in_dir(&install_dir, binary_name) {
        let symlink_path = bin_dir.join(binary_name);
        let _ = std::fs::remove_file(&symlink_path);
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            if symlink(&binary_path, &symlink_path).is_ok() {
                let _ = Command::new("chmod")
                    .args(["+x", &symlink_path.to_string_lossy()])
                    .output();
            }
        }
        #[cfg(windows)]
        {
            let _ = std::fs::copy(&binary_path, &symlink_path);
        }
    }

    Ok(format!("Successfully installed {} v{}", def.name, version))
}

/// 扫描应用残留（预览模式，不执行任何删除）
#[tauri::command]
pub fn scan_app_residues(
    app_name: String,
    package_name: String,
) -> Result<crate::residue_scanner::ResidueScan, String> {
    let scan = crate::residue_scanner::scan_for_residues(&app_name, &package_name);
    Ok(scan)
}

/// 强制卸载：杀死进程 → 包管理器强制卸载 → 深度清理残留
#[tauri::command]
pub async fn force_uninstall_software(
    package_name: String,
    app_name: String,
) -> Result<String, String> {
    use crate::residue_scanner::snapshot;
    use std::time::Duration;

    let mut messages: Vec<String> = Vec::new();
    let name_lower = app_name.to_lowercase();

    // 1) 杀死相关进程
    let killed = kill_processes_by_name(&name_lower);
    if killed > 0 {
        messages.push(format!("Killed {} process(es)", killed));
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // 2) 包管理器强制卸载
    let uninstall_result = software_pm::uninstall_software_exec(package_name.clone()).await;
    match &uninstall_result {
        Ok(msg) => messages.push(msg.clone()),
        Err(e) => messages.push(format!("Package manager removal: {}", e)),
    }

    // 安全校验：包管理器卸载失败时不执行任何目录/文件删除。
    // 残留删除只应在软件确实已从系统中移除后执行，避免误删仍在使用的活动数据。
    if uninstall_result.is_err() {
        messages.push(
            "Uninstall failed; skipped residue cleanup to avoid deleting in-use data.".to_string(),
        );
        return Ok(messages.join("\n"));
    }

    // 3) 获取所有已知的残留路径（含关键词扫描）
    let scan = crate::residue_scanner::scan_for_residues(&app_name, &package_name);

    // 4) 快照记录
    let all_paths: Vec<std::path::PathBuf> = scan
        .directories
        .iter()
        .chain(scan.files.iter())
        .map(|i| std::path::PathBuf::from(&i.path))
        .collect();
    let _before = snapshot::take_snapshot(&all_paths);

    // 5) 记录注册表+服务路径 (Windows)
    #[cfg(target_os = "windows")]
    let _registry_paths: Vec<String> = scan.registry_keys.iter().map(|r| r.path.clone()).collect();

    // 6) 先删文件，再删目录（递归）
    let mut cleaned = Vec::new();
    let mut failed = Vec::new();

    // 删除文件（仅删除标记为安全的）
    for item in &scan.files {
        if !item.is_safe_to_delete {
            failed.push(format!("{} (not marked safe to delete)", item.path));
            continue;
        }
        if let Err(e) = std::fs::remove_file(&item.path) {
            failed.push(format!("{} ({})", item.path, e));
        } else {
            cleaned.push(item.path.clone());
        }
    }

    // 删除目录（仅删除标记为安全的）
    for item in &scan.directories {
        if !item.is_safe_to_delete {
            failed.push(format!("{} (not marked safe to delete)", item.path));
            continue;
        }
        if let Err(e) = std::fs::remove_dir_all(&item.path) {
            failed.push(format!("{} ({})", item.path, e));
        } else {
            cleaned.push(item.path.clone());
        }
    }

    // 7) 清理快捷方式
    for item in &scan.shortcuts {
        if item.is_safe_to_delete {
            if let Err(e) = std::fs::remove_file(&item.path) {
                failed.push(format!("{} ({})", item.path, e));
            } else {
                cleaned.push(item.path.clone());
            }
        }
    }

    // 8) 清理服务文件
    for item in &scan.services {
        if item.is_safe_to_delete {
            if let Err(e) = std::fs::remove_file(&item.path) {
                failed.push(format!("{} ({})", item.path, e));
            } else {
                cleaned.push(item.path.clone());
            }
        }
    }

    // 9) 总结
    let mut result = messages.join("\n");
    result.push_str(&format!("\n\nCleaned {} items", cleaned.len()));
    if !failed.is_empty() {
        result.push_str(&format!("\nFailed to clean {} items", failed.len()));
        for f in failed.iter().take(10) {
            result.push_str(&format!("\n  - {}", f));
        }
        if failed.len() > 10 {
            result.push_str(&format!("\n  ... and {} more", failed.len() - 10));
        }
    }

    Ok(result)
}

/// 仅清理指定的残留项目（不执行卸载），用于用户在扫描预览后选择性清理
#[tauri::command]
pub fn clean_specific_residues(items: Vec<String>) -> Result<String, String> {
    let mut cleaned = Vec::new();
    let mut failed = Vec::new();

    for path in &items {
        let p = std::path::Path::new(path);
        if !p.exists() {
            continue;
        }
        if p.is_dir() {
            if let Err(e) = std::fs::remove_dir_all(p) {
                failed.push(format!("{} ({})", path, e));
            } else {
                cleaned.push(path.clone());
            }
        } else {
            if let Err(e) = std::fs::remove_file(p) {
                failed.push(format!("{} ({})", path, e));
            } else {
                cleaned.push(path.clone());
            }
        }
    }

    let mut result = String::new();
    if !cleaned.is_empty() {
        result.push_str(&format!("Cleaned {} item(s)", cleaned.len()));
    }
    if !failed.is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&format!("Failed: {} item(s)", failed.len()));
        for f in failed.iter().take(10) {
            result.push_str(&format!("\n  - {}", f));
        }
    }
    if result.is_empty() {
        result.push_str("No items to clean.");
    }
    Ok(result)
}

/// 进程名/可执行名是否与关键词匹配（M3 修复：收紧匹配，避免误杀无关进程）
///
/// 匹配规则（任一命中即匹配）：
/// 1. 完全相等（如 `idea` == `idea`）；
/// 2. 关键词 + 数字/版本后缀（如 `idea` → `idea64`、`node` → `nodejs20`）；
/// 3. 长关键词（≥4 字符）在名称中的完整单词匹配（`idea` 命中 `intellij-idea`，但不命中 `ideally`）。
///
/// 相比旧的 `contains` 子串匹配，这能避免 `idea` 误杀名称里恰好含 "idea" 的无关进程。
fn process_matches_keyword(target: &str, kw: &str) -> bool {
    if target == kw {
        return true;
    }
    // 关键词 + 版本/数字后缀：node → nodejs、node20、code-oss
    if let Some(rest) = target.strip_prefix(kw) {
        let first = rest.chars().next().unwrap_or(' ');
        if first.is_ascii_digit() || !(first.is_ascii_alphanumeric() || first == '_') {
            return true;
        }
    }
    // 长关键词才允许词内匹配（避免短关键词过度匹配）
    if kw.len() >= 4 && contains_whole_word(target, kw) {
        return true;
    }
    false
}

/// target 中 kw 以单词边界出现（前后不是字母/数字/下划线）
fn contains_whole_word(target: &str, kw: &str) -> bool {
    let bytes = target.as_bytes();
    let mut from = 0;
    while let Some(rel) = target[from..].find(kw) {
        let abs = from + rel;
        let before_ok = abs == 0 || !is_word_byte(bytes[abs - 1]);
        let end = abs + kw.len();
        let after_ok = end >= bytes.len() || !is_word_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = abs + 1;
    }
    false
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// 按名称关键词杀死匹配的进程（跨平台）
fn kill_processes_by_name(name_lower: &str) -> usize {
    #[cfg(unix)]
    use sysinfo::Signal;
    use sysinfo::System;
    let mut system = System::new();
    system.refresh_all();

    let keywords: Vec<String> = name_lower
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
        .filter(|s| s.len() >= 3)
        .map(|s| s.to_lowercase())
        .collect();

    if keywords.is_empty() {
        return 0;
    }

    let mut killed = 0;
    for process in system.processes().values() {
        let pname = process.name().to_string_lossy().to_lowercase();
        let exe = process
            .exe()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 收紧后的关键词匹配（M3 修复）
        let matches = keywords
            .iter()
            .any(|kw| process_matches_keyword(&pname, kw) || process_matches_keyword(&exe, kw));
        if !matches {
            continue;
        }

        // 跳过自身
        if let Ok(cur) = std::env::current_exe() {
            if let Some(cur_name) = cur.file_stem().and_then(|s| s.to_str()) {
                if process_matches_keyword(&pname, &cur_name.to_lowercase()) {
                    continue;
                }
            }
        }

        #[cfg(unix)]
        {
            if process.kill_with(Signal::Term).is_some()
                || process.kill_with(Signal::Kill).is_some()
            {
                killed += 1;
            }
        }
        #[cfg(windows)]
        {
            if process.kill() {
                killed += 1;
            }
        }
    }
    killed
}

#[cfg(test)]
mod tests {
    use super::software_data::map_package_name;
    use super::*;

    // ============ parse_pm_list_output ============

    #[test]
    fn test_parse_apt_output() {
        let output = "Listing...\nfoo/stable,now 1.2.3 amd64 [installed]\nbar/stable 0.5.0 amd64\nbaz/stable,now 3.0.0 all [installed,automatic]\n";
        let apps = parse_pm_list_output("apt", output);
        // 只保留手动安装（[installed]），跳过 automatic 依赖
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0], ("foo".to_string(), "1.2.3".to_string()));
    }

    #[test]
    fn test_parse_pacman_output() {
        let output = "foo 1.2.3\nbar 0.5.0\nlocal/baz 3.0.0\n";
        let apps = parse_pm_list_output("pacman", output);
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[0], ("foo".to_string(), "1.2.3".to_string()));
        assert_eq!(apps[1], ("bar".to_string(), "0.5.0".to_string()));
        assert_eq!(apps[2], ("baz".to_string(), "3.0.0".to_string()));
    }

    #[test]
    fn test_parse_dnf_output() {
        let output = "\nInstalled Packages\nfoo.x86_64 1.2.3 @repo\nbar.noarch 0.5.0 @other\nAvailable Packages\nbaz.x86_64 4.0.0 @extra\n";
        let apps = parse_pm_list_output("dnf", output);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0], ("foo".to_string(), "1.2.3".to_string()));
        assert_eq!(apps[1], ("bar".to_string(), "0.5.0".to_string()));
    }

    #[test]
    fn test_parse_homebrew_output() {
        let output = "foo 1.2.3\nbar 0.5.0\n";
        let apps = parse_pm_list_output("Homebrew", output);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0], ("foo".to_string(), "1.2.3".to_string()));
        assert_eq!(apps[1], ("bar".to_string(), "0.5.0".to_string()));
    }

    #[test]
    fn test_parse_winget_output() {
        let output = "Name Id Version Available Source\n--- --- --- --- ---\nFoo Foo.Id 1.2.3 winget\nBar Bar.Id 0.5.0 winget\n";
        let apps = parse_pm_list_output("winget", output);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0], ("Foo".to_string(), "1.2.3".to_string()));
        assert_eq!(apps[1], ("Bar".to_string(), "0.5.0".to_string()));
    }

    #[test]
    fn test_parse_snap_output() {
        let output = "Name Version Rev Tracking Publisher Notes\ncore20 2024-01-01 1234 latest/stable canonical✓ -\nfirefox 123.0 5678 latest/stable mozilla✓ -\n";
        let apps = parse_pm_list_output("snap", output);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0], ("core20".to_string(), "2024-01-01".to_string()));
        assert_eq!(apps[1], ("firefox".to_string(), "123.0".to_string()));
    }

    #[test]
    fn test_parse_flatpak_output() {
        let output = "ApplicationID Version Branch Origin\norg.mozilla.firefox 123.0 stable flathub\norg.gimp.GIMP 3.0 stable flathub\n";
        let apps = parse_pm_list_output("flatpak", output);
        assert_eq!(apps.len(), 2);
        assert_eq!(
            apps[0],
            ("org.mozilla.firefox".to_string(), "123.0".to_string())
        );
        assert_eq!(apps[1], ("org.gimp.GIMP".to_string(), "3.0".to_string()));
    }

    #[test]
    fn test_parse_apk_output() {
        let output = "foo-1.2.3 x86_64 {foo} [instited]\nbar-0.5.0 x86_64 {bar} [installed]\n";
        let apps = parse_pm_list_output("apk", output);
        // only [installed] lines are parsed, not [instited]
        assert_eq!(apps.len(), 1);
        // "bar-0.5.0" - the parser requires prev char before '-' to be digit/dot
        // 'r' is not digit, so it falls through to the full-name fallback
        assert_eq!(apps[0], ("bar-0.5.0".to_string(), "installed".to_string()));
    }

    #[test]
    fn test_parse_zypper_output() {
        // zypper se --installed-only format: Status | Name | Summary | Version | Arch | Repository
        // i+ = 手动安装（保留）；i = 自动安装（跳过，依赖）
        let output = "S | Name | Summary | Type\n--+------+---------+-----\ni+ | foo | Foo pkg | 1.2.3 | x86_64 | repo\ni | bar | Bar pkg | 0.5.0 | noarch | repo\n";
        let apps = parse_pm_list_output("zypper", output);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0], ("foo".to_string(), "1.2.3".to_string()));
    }

    #[test]
    fn test_parse_unknown_pm() {
        let apps = parse_pm_list_output("unknown-pm", "some output");
        assert!(apps.is_empty());
    }

    // ============ map_package_name ============

    #[test]
    fn test_map_vscode() {
        assert_eq!(
            map_package_name("code", "winget"),
            "Microsoft.VisualStudioCode"
        );
        assert_eq!(map_package_name("code", "apt"), "code");
        assert_eq!(map_package_name("code", "brew"), "visual-studio-code");
    }

    #[test]
    fn test_map_nodejs() {
        assert_eq!(map_package_name("nodejs", "apt"), "nodejs");
        assert_eq!(map_package_name("nodejs", "brew"), "node");
        assert_eq!(map_package_name("nodejs", "winget"), "OpenJS.NodeJS.LTS");
    }

    #[test]
    fn test_map_python() {
        assert_eq!(map_package_name("python3", "apt"), "python3");
        assert_eq!(map_package_name("python3", "brew"), "python");
        assert_eq!(map_package_name("python3", "winget"), "Python.Python.3.12");
    }

    #[test]
    fn test_map_golang() {
        assert_eq!(map_package_name("golang", "apt"), "golang");
        assert_eq!(map_package_name("golang", "brew"), "go");
        assert_eq!(map_package_name("golang", "winget"), "GoLang.Go");
    }

    #[test]
    fn test_map_git() {
        assert_eq!(map_package_name("git", "apt"), "git");
        assert_eq!(map_package_name("git", "brew"), "git");
        assert_eq!(map_package_name("git", "winget"), "Git.Git");
    }

    #[test]
    fn test_map_unknown() {
        assert_eq!(map_package_name("unknown-pkg", "apt"), "unknown-pkg");
        assert_eq!(map_package_name("git", "unknown-pm"), "git");
    }

    // ============ get_pm_list_args ============

    #[test]
    fn test_get_pm_list_args_all() {
        assert_eq!(
            get_pm_list_args("apt"),
            Some(&["list", "--installed"] as &[&str])
        );
        assert_eq!(get_pm_list_args("pacman"), Some(&["-Qe"] as &[&str]));
        assert_eq!(
            get_pm_list_args("dnf"),
            Some(&["list", "installed"] as &[&str])
        );
        assert_eq!(
            get_pm_list_args("Homebrew"),
            Some(&["list", "--formula", "--versions"] as &[&str])
        );
        assert_eq!(
            get_pm_list_args("winget"),
            Some(&["list", "--accept-source-agreements"] as &[&str])
        );
        assert_eq!(get_pm_list_args("snap"), Some(&["list"] as &[&str]));
        assert_eq!(
            get_pm_list_args("flatpak"),
            Some(&[
                "list",
                "--app",
                "--columns=application,version,branch,origin"
            ] as &[&str])
        );
        assert_eq!(
            get_pm_list_args("chocolatey"),
            Some(&["list", "--local-only"] as &[&str])
        );
        assert_eq!(get_pm_list_args("unknown"), None);
    }

    #[test]
    fn test_version_rejects_path_traversal() {
        assert!(is_valid_version("1.2.3"));
        assert!(is_valid_version("v24.2.1"));
        assert!(!is_valid_version("../evil"));
        assert!(!is_valid_version("1.2.3/../../etc"));
        assert!(!is_valid_version("a;b"));
        assert!(!is_valid_version(""));
        assert!(!is_valid_version(&"v".repeat(200)));
    }

    // ============ icon resolution ============

    #[cfg(target_os = "linux")]
    #[test]
    fn test_desktop_name_of() {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("dnx_icon_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("test.desktop");
        fs::write(
            &f,
            "[Desktop Entry]\nName=My Cool App\nName[zh]=我的应用\nIcon=myapp\n",
        )
        .unwrap();
        assert_eq!(desktop_name_of(&f), "My Cool App");
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_desktop_icon_path_absolute() {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("dnx_icon_test_abs_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let png = dir.join("icon.png");
        fs::write(&png, b"\x89PNG\r\n\x1a\n").unwrap();
        let desktop = dir.join("app.desktop");
        fs::write(
            &desktop,
            format!("[Desktop Entry]\nName=App\nIcon={}\n", png.display()),
        )
        .unwrap();
        let got = desktop_icon_path(&desktop);
        assert!(got.is_some(), "should resolve absolute icon path");
        assert_eq!(got.unwrap(), png);
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_read_image_as_data_url_png() {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("dnx_icon_test_url_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        fs::write(&png, b"\x89PNG\r\n\x1a\n1234").unwrap();
        let got = read_image_as_data_url(&png);
        assert!(got.is_some());
        assert!(got.unwrap().starts_with("data:image/png;base64,"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_read_image_as_data_url_unsupported_ext() {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("dnx_icon_test_bad_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let txt = dir.join("a.txt");
        fs::write(&txt, b"hello").unwrap();
        assert!(read_image_as_data_url(&txt).is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
