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
                install_args: &["install", "--silent"],
                uninstall_args: &["uninstall", "--silent"],
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
    let mut cmd = std::process::Command::new("pkexec");
    cmd.arg(binary);
    cmd.args(args);
    cmd.output()
        .map_err(|e| format!("Failed to execute pkexec: {}", e))
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
        let mut last_error = String::new();

        for pm in &managers_clone {
            let pkg = map_package_name(&pkg_name, pm.name);
            let mut args: Vec<&str> = pm.install_args.to_vec();
            args.push(pkg);

            let output = if pm.needs_sudo {
                run_elevated(pm.binary, &args)?
            } else {
                Command::new(pm.binary)
                    .args(&args)
                    .output()
                    .map_err(|e| format!("Failed to execute {}: {}", pm.binary, e))?
            };

            if output.status.success() {
                return Ok(format!(
                    "Successfully installed {} via {}",
                    pkg_name, pm.name
                ));
            }
            last_error = String::from_utf8_lossy(&output.stderr).to_string();
        }

        Err(format!(
            "Failed to install {} with all package managers. Last error: {}",
            pkg_name,
            last_error.trim()
        ))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// 卸载软件执行体（跨平台，多包管理器支持）
/// 命令入口 uninstall_software 位于 software.rs，仅做透传
pub(crate) async fn uninstall_software_exec(package_name: String) -> Result<String, String> {
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
        let mut last_error = String::new();

        for pm in &managers_clone {
            let pkg = map_package_name(&pkg_name, pm.name);
            let mut args: Vec<&str> = pm.uninstall_args.to_vec();
            args.push(pkg);

            let output = if pm.needs_sudo {
                run_elevated(pm.binary, &args)?
            } else {
                Command::new(pm.binary)
                    .args(&args)
                    .output()
                    .map_err(|e| format!("Failed to execute {}: {}", pm.binary, e))?
            };

            if output.status.success() {
                return Ok(format!(
                    "Successfully uninstalled {} via {}",
                    pkg_name, pm.name
                ));
            }
            last_error = String::from_utf8_lossy(&output.stderr).to_string();
        }

        Err(format!(
            "Failed to uninstall {} with all package managers. Last error: {}",
            pkg_name,
            last_error.trim()
        ))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}
