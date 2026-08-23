/// 包管理器安装/卸载执行体（从 software.rs 拆分出来）
///
/// 包含：
/// - PackageManager 定义与 detect_package_managers 检测
/// - run_elevated（macOS osascript / Linux pkexec / Windows 直调）
/// - install_software / uninstall_software 命令执行体
use std::process::Command;

use super::software_data::map_package_name;

#[derive(Debug, Clone)]
pub(super) struct PackageManager {
    pub name: &'static str,
    pub binary: &'static str,
    pub needs_sudo: bool,
    pub install_args: &'static [&'static str],   // 不含包名
    pub uninstall_args: &'static [&'static str], // 不含包名
}

/// 检测系统可用的包管理器（按优先级排列）
pub(super) fn detect_package_managers() -> Vec<PackageManager> {
    let mut managers = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if which::which("brew").is_ok() {
            managers.push(PackageManager {
                name: "Homebrew",
                binary: "brew",
                needs_sudo: false,
                install_args: &["install"],
                uninstall_args: &["uninstall"],
            });
        }
        if which::which("port").is_ok() {
            managers.push(PackageManager {
                name: "MacPorts",
                binary: "port",
                needs_sudo: true,
                install_args: &["install"],
                uninstall_args: &["uninstall"],
            });
        }
    }

    #[cfg(target_os = "linux")]
    {
        // 优先级: apt → dnf → pacman → zypper → apk
        if which::which("apt").is_ok() {
            managers.push(PackageManager {
                name: "apt",
                binary: "apt",
                needs_sudo: true,
                install_args: &["install", "-y"],
                uninstall_args: &["remove", "-y"],
            });
        }
        if which::which("dnf").is_ok() {
            managers.push(PackageManager {
                name: "dnf",
                binary: "dnf",
                needs_sudo: true,
                install_args: &["install", "-y"],
                uninstall_args: &["remove", "-y"],
            });
        }
        if which::which("pacman").is_ok() {
            managers.push(PackageManager {
                name: "pacman",
                binary: "pacman",
                needs_sudo: true,
                install_args: &["-S", "--noconfirm"],
                uninstall_args: &["-R", "--noconfirm"],
            });
        }
        if which::which("zypper").is_ok() {
            managers.push(PackageManager {
                name: "zypper",
                binary: "zypper",
                needs_sudo: true,
                install_args: &["install", "-y"],
                uninstall_args: &["remove", "-y"],
            });
        }
        if which::which("apk").is_ok() {
            managers.push(PackageManager {
                name: "apk",
                binary: "apk",
                needs_sudo: true,
                install_args: &["add"],
                uninstall_args: &["del"],
            });
        }
        if which::which("snap").is_ok() {
            managers.push(PackageManager {
                name: "snap",
                binary: "snap",
                needs_sudo: true,
                install_args: &["install"],
                uninstall_args: &["remove"],
            });
        }
        if which::which("flatpak").is_ok() {
            managers.push(PackageManager {
                name: "flatpak",
                binary: "flatpak",
                needs_sudo: false,
                install_args: &["install", "-y"],
                uninstall_args: &["uninstall", "-y"],
            });
        }
    }

    #[cfg(target_os = "windows")]
    {
        if which::which("winget").is_ok() {
            managers.push(PackageManager {
                name: "winget",
                binary: "winget",
                needs_sudo: false,
                // --accept-source-agreements 避免 winget 在无人值守时挂起等待用户确认
                install_args: &["install", "--silent", "--accept-source-agreements"],
                uninstall_args: &["uninstall", "--silent", "--accept-source-agreements"],
            });
        }
        if which::which("choco").is_ok() {
            managers.push(PackageManager {
                name: "chocolatey",
                binary: "choco",
                needs_sudo: false,
                install_args: &["install", "-y"],
                uninstall_args: &["uninstall", "-y"],
            });
        }
    }

    managers
}

#[cfg(target_os = "macos")]
mod elevated {
    pub(super) fn run(binary: &str, args: &[&str]) -> Result<std::process::Output, String> {
        let mut script = String::from("do shell script \"");
        script.push_str(&shell_escape(binary));
        for a in args {
            script.push(' ');
            script.push_str(&shell_escape(a));
        }
        script.push_str("\" with administrator privileges");

        std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("Failed to execute osascript: {}", e))
    }

    fn shell_escape(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '$' => out.push_str("\\$"),
                '`' => out.push_str("\\`"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                other => out.push(other),
            }
        }
        out.push('"');
        out
    }
}

#[cfg(target_os = "macos")]
fn run_elevated(binary: &str, args: &[&str]) -> Result<std::process::Output, String> {
    elevated::run(binary, args)
}

#[cfg(target_os = "linux")]
fn run_elevated(binary: &str, args: &[&str]) -> Result<std::process::Output, String> {
    // 优先 pkexec（图形环境 polkit 弹窗），若不存在则回退到 sudo -n（非交互）
    if which::which("pkexec").is_ok() {
        let mut cmd = std::process::Command::new("pkexec");
        cmd.arg(binary);
        cmd.args(args);
        if let Ok(out) = cmd.output() {
            // pkexec 被用户取消会返回 127/126，但仍视为执行结果；仅在 spawn 失败时降级
            return Ok(out);
        }
        // pkexec spawn 失败（极少见），继续尝试 sudo
    }
    if which::which("sudo").is_ok() {
        let mut cmd = std::process::Command::new("sudo");
        cmd.arg("-n");
        cmd.arg(binary);
        cmd.args(args);
        return cmd
            .output()
            .map_err(|e| format!("Failed to execute sudo: {}", e));
    }
    // 无提权工具可用，直接尝试执行（适用于已 root 的容器/CI）
    std::process::Command::new(binary)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", binary, e))
}

#[cfg(target_os = "windows")]
fn run_elevated(binary: &str, args: &[&str]) -> Result<std::process::Output, String> {
    // Windows 包管理器 (winget, choco) 的 needs_sudo 均为 false，
    // 此函数仅用于编译通过，实际不会被调用。
    std::process::Command::new(binary)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", binary, e))
}

/// 安装软件执行体（跨平台，多包管理器支持）
/// 命令入口 install_software 位于 software.rs，仅做透传
pub(crate) async fn install_software_exec(package_name: String) -> Result<String, String> {
    let managers = detect_package_managers();

    if managers.is_empty() {
        return Err("No supported package manager found on this system.\n\nTo use the Software Center, please install a package manager:\n- macOS: Install Homebrew -> https://brew.sh/\n- Linux: Your distro likely has apt/dnf/pacman/zypper/apk pre-installed\n- Windows: winget comes built-in with Win 11 / Win 10 1809+. Chocolatey: https://chocolatey.org/install".to_string());
    }

    let managers_clone: Vec<_> = managers
        .iter()
        .map(|pm| PackageManager {
            name: pm.name,
            binary: pm.binary,
            needs_sudo: pm.needs_sudo,
            install_args: pm.install_args,
            uninstall_args: pm.uninstall_args,
        })
        .collect();
    let pkg_name = package_name.clone();

    tokio::task::spawn_blocking(move || {
        let mut errors: Vec<String> = Vec::new();

        for pm in &managers_clone {
            let pkg = map_package_name(&pkg_name, pm.name);
            let mut args: Vec<&str> = pm.install_args.to_vec();
            // Homebrew cask 需额外 --cask（否则 formula 查找失败）
            let needs_cask = pm.name == "Homebrew" && brew_needs_cask(&pkg_name);
            if needs_cask && !args.contains(&"--cask") {
                args.push("--cask");
            }
            args.push(pkg);

            // L1 修复：单次执行失败（含 pkexec 无法启动等）仅记录错误并继续尝试
            // 下一个包管理器，不再用 `?` 中断整个循环。
            let output = if pm.needs_sudo {
                match run_elevated(pm.binary, &args) {
                    Ok(o) => o,
                    Err(e) => {
                        errors.push(format!("{}: {}", pm.name, e));
                        continue;
                    }
                }
            } else {
                match Command::new(pm.binary).args(&args).output() {
                    Ok(o) => o,
                    Err(e) => {
                        errors.push(format!(
                            "{}: Failed to execute {}: {}",
                            pm.name, pm.binary, e
                        ));
                        continue;
                    }
                }
            };

            if output.status.success() {
                return Ok(format!(
                    "Successfully installed {} via {}",
                    pkg_name, pm.name
                ));
            }
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            errors.push(format!("{}: {}", pm.name, detail));
        }

        Err(format!(
            "Failed to install {} with all package managers. Errors: {}",
            pkg_name,
            errors.join(" | ")
        ))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// 卸载软件执行体（跨平台，多包管理器支持）
/// 命令入口 uninstall_software 位于 software.rs，仅做透传
pub(crate) async fn uninstall_software_exec(package_name: String) -> Result<String, String> {
    uninstall_with_source_hint(package_name, None).await
}

/// 带来源提示的卸载（AppUninstaller 传入 source 可定向到对应包管理器，避免全量轮询）
pub(crate) async fn uninstall_with_source_hint(
    package_name: String,
    source_hint: Option<String>,
) -> Result<String, String> {
    let managers = detect_package_managers();

    if managers.is_empty() {
        return Err("No supported package manager found on this system.\n\nTo use the Software Center, please install a package manager:\n- macOS: Install Homebrew -> https://brew.sh/\n- Linux: Your distro likely has apt/dnf/pacman/zypper/apk pre-installed\n- Windows: winget comes built-in with Win 11 / Win 10 1809+. Chocolatey: https://chocolatey.org/install".to_string());
    }

    // 若有来源提示，优先尝试对应 PM（大小写不敏感），再回退全量
    let mut ordered: Vec<PackageManager> = Vec::new();
    if let Some(ref hint) = source_hint {
        let h = hint.to_lowercase();
        for pm in &managers {
            if pm.name.to_lowercase() == h || pm.binary.to_lowercase() == h {
                ordered.push(PackageManager {
                    name: pm.name,
                    binary: pm.binary,
                    needs_sudo: pm.needs_sudo,
                    install_args: pm.install_args,
                    uninstall_args: pm.uninstall_args,
                });
            }
        }
    }
    for pm in &managers {
        let already = ordered.iter().any(|o| o.name == pm.name);
        if !already {
            ordered.push(PackageManager {
                name: pm.name,
                binary: pm.binary,
                needs_sudo: pm.needs_sudo,
                install_args: pm.install_args,
                uninstall_args: pm.uninstall_args,
            });
        }
    }

    let pkg_name = package_name.clone();
    let hint_clone = source_hint.clone();

    tokio::task::spawn_blocking(move || {
        let mut errors: Vec<String> = Vec::new();

        for pm in &ordered {
            // AppUninstaller 传入的已是真实包 ID（如 flatpak 的 org.mozilla.firefox），
            // 若 source_hint 与当前 PM 匹配则直接使用原始包名，避免 map_package_name 回退到错误映射
            let is_direct = hint_clone
                .as_ref()
                .map(|h| {
                    h.to_lowercase() == pm.name.to_lowercase()
                        || h.to_lowercase() == pm.binary.to_lowercase()
                })
                .unwrap_or(false);
            let pkg: &str = if is_direct {
                &pkg_name
            } else {
                map_package_name(&pkg_name, pm.name)
            };
            let mut args: Vec<&str> = pm.uninstall_args.to_vec();
            let needs_cask = pm.name == "Homebrew" && brew_needs_cask(&pkg_name);
            if needs_cask && !args.contains(&"--cask") {
                args.push("--cask");
            }
            args.push(pkg);

            // L1 修复：单次执行失败仅记录错误并继续尝试下一个包管理器
            let output = if pm.needs_sudo {
                match run_elevated(pm.binary, &args) {
                    Ok(o) => o,
                    Err(e) => {
                        errors.push(format!("{}: {}", pm.name, e));
                        continue;
                    }
                }
            } else {
                match Command::new(pm.binary).args(&args).output() {
                    Ok(o) => o,
                    Err(e) => {
                        errors.push(format!(
                            "{}: Failed to execute {}: {}",
                            pm.name, pm.binary, e
                        ));
                        continue;
                    }
                }
            };

            if output.status.success() {
                return Ok(format!(
                    "Successfully uninstalled {} via {}",
                    pkg_name, pm.name
                ));
            }
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            // 忽略“未安装/找不到包”这类预期错误，继续尝试下一个 PM，错误暂存但不立即展示
            errors.push(format!("{}: {}", pm.name, detail));
        }

        // PM 全失败后，尝试清理 DevNexus 自托管安装目录（install_software_from_url 产物）
        if let Some(msg) = try_remove_managed_install(&pkg_name) {
            return Ok(msg);
        }

        Err(format!(
            "Failed to uninstall {} with all package managers. Errors: {}",
            pkg_name,
            if errors.is_empty() {
                "no package manager matched".to_string()
            } else {
                errors.join(" | ")
            }
        ))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// 判断 Homebrew 包是否为 cask（需 --cask 标志）
fn brew_needs_cask(generic: &str) -> bool {
    matches!(
        generic,
        "code"
            | "sublime-text"
            | "postman"
            | "dbeaver-ce"
            | "intellij-idea-community"
            | "mysql-workbench"
            | "tableplus"
            | "docker-desktop"
            | "zed"
    )
}

/// 尝试移除 DevNexus 自托管安装（~/.local/share/devnexus/software/<pkg>/*）
/// 返回 Some 成功消息，无对应目录则返回 None 供外层继续报错
fn try_remove_managed_install(pkg_name: &str) -> Option<String> {
    // 校验包名仅含安全字符，避免路径穿越
    if pkg_name.is_empty()
        || pkg_name.len() > 128
        || !pkg_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return None;
    }
    let base = get_managed_base_dir()?;
    let pkg_dir = base.join(pkg_name);
    if !pkg_dir.exists() || !pkg_dir.is_dir() {
        return None;
    }
    // 同时清理 bin 符号链接
    let bin_link = base.join("bin").join(pkg_name);
    // 对于名称映射的二进制（如 nodejs->node），也尝试清理别名链接
    let aliases: &[(&str, &str)] = &[
        ("nodejs", "node"),
        ("python3", "python3"),
        ("golang", "go"),
        ("code", "code"),
        ("neovim", "nvim"),
    ];
    let mut removed_bins = Vec::new();
    if bin_link.exists() {
        let _ = std::fs::remove_file(&bin_link);
        removed_bins.push(bin_link.display().to_string());
    }
    for (orig, alias) in aliases {
        if *orig == pkg_name {
            let p = base.join("bin").join(alias);
            if p.exists() && p != bin_link {
                let _ = std::fs::remove_file(&p);
                removed_bins.push(p.display().to_string());
            }
        }
    }
    match std::fs::remove_dir_all(&pkg_dir) {
        Ok(_) => {
            let mut msg = format!("Removed managed install {}", pkg_dir.display());
            if !removed_bins.is_empty() {
                msg.push_str(&format!(" and cleaned {}", removed_bins.join(", ")));
            }
            Some(msg)
        }
        Err(e) => Some(format!(
            "Found managed install at {} but failed to remove: {}",
            pkg_dir.display(),
            e
        )),
    }
}

fn get_managed_base_dir() -> Option<std::path::PathBuf> {
    let base = if cfg!(target_os = "macos") {
        std::env::var("HOME").ok().map(|h| {
            std::path::PathBuf::from(h).join("Library/Application Support/devnexus/software")
        })
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join("devnexus/software"))
    } else {
        std::env::var("HOME")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join(".local/share/devnexus/software"))
    }?;
    Some(base)
}
