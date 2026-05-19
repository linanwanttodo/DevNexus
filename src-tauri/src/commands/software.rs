use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Clone)]
pub struct Software {
    pub name: String,
    pub category: String,
    pub version: String,
    pub status: String,
    pub action: String,
    pub package_name: Option<String>, // 包管理器的包名
}

/// GUI 应用名单：这些程序不支持 --version，执行会直接启动 GUI，跳过版本检测
const GUI_APPS: &[&str] = &[
    "postman", "dbeaver", "dbeaver-ce", "mysql-workbench",
    "code", "docker", "docker-ce",
];

/// 安全获取软件版本：对 GUI 应用跳过，避免启动它们
fn safe_get_version(cmd: &str) -> String {
    if GUI_APPS.contains(&cmd) {
        return "installed".to_string();
    }

    match std::process::Command::new(cmd)
        .arg("--version")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            use std::sync::mpsc;
            let (tx, rx) = mpsc::channel();
            let stdout = child.stdout.take();
            std::thread::spawn(move || {
                let _ = tx.send(child.wait());
            });
            match rx.recv_timeout(std::time::Duration::from_secs(3)) {
                Ok(Ok(status)) if status.success() => {
                    if let Some(mut stdout) = stdout {
                        use std::io::Read;
                        let mut buf = Vec::new();
                        let _ = std::io::BufReader::new(&mut stdout).read_to_end(&mut buf);
                        let ver = String::from_utf8_lossy(&buf);
                        let first_line = ver.lines().next().unwrap_or("unknown");
                        if first_line.len() > 60 {
                            first_line[..57].to_string() + "..."
                        } else {
                            first_line.to_string()
                        }
                    } else {
                        "unknown".to_string()
                    }
                }
                _ => "timeout".to_string(),
            }
        }
        Err(_) => "unknown".to_string(),
    }
}

/// 检测已安装的软件（不执行命令，避免启动 GUI 程序）
fn detect_software(name: &str, cmd: &str, category: &str, package_name: &str) -> Software {
    let found = which::which(cmd).is_ok()
        || check_common_paths(cmd);

    let status = if found { "installed" } else { "available" };

    let version = if found {
        safe_get_version(cmd)
    } else {
        "N/A".to_string()
    };

    let action = if found { "Open" } else { "Install" };

    Software {
        name: name.to_string(),
        category: category.to_string(),
        version,
        status: status.to_string(),
        action: action.to_string(),
        package_name: Some(package_name.to_string()),
    }
}

/// 检查常见安装路径（nvm、snap、/usr/local 等）
fn check_common_paths(cmd: &str) -> bool {
    #[cfg(unix)]
    {
        let paths = [
            format!("/usr/local/bin/{}", cmd),
            format!("/opt/homebrew/bin/{}", cmd),
            format!("/snap/bin/{}", cmd),
        ];

        if cmd == "node" || cmd == "npm" {
            if let Ok(home) = std::env::var("HOME") {
                let nvm_base = format!("{}/.nvm/versions/node", home);
                if let Ok(entries) = std::fs::read_dir(&nvm_base) {
                    for entry in entries.flatten() {
                        let bin = entry.path().join("bin").join(cmd);
                        if bin.exists() {
                            return true;
                        }
                    }
                }
            }
        }

        for path in &paths {
            if std::path::Path::new(path).exists() {
                return true;
            }
        }
    }

    #[cfg(windows)]
    {
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            let nvm_base = format!("{}\\nvm", localappdata);
            if let Ok(entries) = std::fs::read_dir(&nvm_base) {
                for entry in entries.flatten() {
                    let bin = entry.path().join(cmd);
                    if bin.exists() {
                        return true;
                    }
                }
            }
        }
        if let Ok(programfiles) = std::env::var("ProgramFiles") {
            let paths = [
                format!("{}\\{}", programfiles, cmd),
            ];
            for path in &paths {
                if std::path::Path::new(path).exists() {
                    return true;
                }
            }
        }
    }

    false
}

#[tauri::command]
pub fn list_software() -> Vec<Software> {
    let mut list = Vec::new();

    // ============ IDEs & Editors ============
    list.push(detect_software("Visual Studio Code", "code", "ide", "code"));
    list.push(detect_software("Neovim", "nvim", "ide", "neovim"));
    list.push(detect_software("Vim", "vim", "ide", "vim"));
    list.push(detect_software("Sublime Text", "subl", "ide", "sublime-text"));
    list.push(detect_software("Zed", "zed", "ide", "zed"));
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        list.push(detect_software("Postman", "postman", "ide", "postman"));
        list.push(detect_software("IntelliJ IDEA Community", "idea", "ide", "intellij-idea-community"));
    }

    // ============ Databases ============
    list.push(detect_software("DBeaver Community", "dbeaver", "database", "dbeaver-ce"));
    list.push(detect_software("SQLite", "sqlite3", "database", "sqlite"));
    list.push(detect_software("PostgreSQL Client", "psql", "database", "postgresql-client"));
    list.push(detect_software("Redis", "redis-cli", "database", "redis"));
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        list.push(detect_software("MySQL Workbench", "mysql-workbench", "database", "mysql-workbench"));
        list.push(detect_software("TablePlus", "tableplus", "database", "tableplus"));
    }

    // ============ CLI Tools ============
    list.push(detect_software("Git", "git", "cli", "git"));
    list.push(detect_software("curl", "curl", "cli", "curl"));
    list.push(detect_software("wget", "wget", "cli", "wget"));
    list.push(detect_software("OpenSSH Client", "ssh", "cli", "openssh-client"));
    list.push(detect_software("GCC", "gcc", "cli", "gcc"));
    list.push(detect_software("Clang", "clang", "cli", "clang"));
    list.push(detect_software("CMake", "cmake", "cli", "cmake"));
    list.push(detect_software("htop", "htop", "cli", "htop"));
    list.push(detect_software("tmux", "tmux", "cli", "tmux"));
    list.push(detect_software("ripgrep", "rg", "cli", "ripgrep"));
    list.push(detect_software("fd", "fd", "cli", "fd-find"));
    list.push(detect_software("jq", "jq", "cli", "jq"));
    list.push(detect_software("fzf", "fzf", "cli", "fzf"));
    #[cfg(target_os = "linux")]
    {
        list.push(detect_software("GParted", "gparted", "cli", "gparted"));
    }

    // ============ Runtimes & Package Managers ============
    list.push(detect_software("Node.js", "node", "runtime", "nodejs"));
    list.push(detect_software("Python 3", "python3", "runtime", "python3"));
    list.push(detect_software("Go", "go", "runtime", "golang"));
    list.push(detect_software("Rust", "rustc", "runtime", "rust"));
    list.push(detect_software("Ruby", "ruby", "runtime", "ruby"));
    list.push(detect_software("Java (JDK)", "java", "runtime", "openjdk-17-jdk"));
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        list.push(detect_software("Docker Desktop", "docker", "runtime", "docker-desktop"));
    }
    #[cfg(target_os = "linux")]
    {
        list.push(detect_software("Docker Engine", "docker", "runtime", "docker-ce"));
    }

    list
}

// ==================== 包管理器检测与包名映射 ====================

#[derive(Debug, Clone)]
struct PackageManager {
    name: &'static str,
    binary: &'static str,
    needs_sudo: bool,
    install_args: &'static [&'static str],  // 不含包名
    uninstall_args: &'static [&'static str], // 不含包名
}

/// 检测系统可用的包管理器（按优先级排列）
fn detect_package_managers() -> Vec<PackageManager> {
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

/// 将通用包名映射为目标包管理器的实际包名
fn map_package_name<'a>(generic: &'a str, pm_name: &str) -> &'a str {
    match (generic, pm_name) {
        // VS Code
        ("code", "winget") => "Microsoft.VisualStudioCode",
        ("code", "chocolatey") => "vscode",
        // Node.js
        ("nodejs", "dnf") => "nodejs",
        ("nodejs", "pacman") => "nodejs",
        // Docker
        ("docker-ce", "pacman") => "docker",
        ("docker-ce", "dnf") => "docker-ce",
        // Python
        ("python3", "pacman") => "python",
        ("python3", "apt") => "python3",
        ("python3", "dnf") => "python3",
        // htop
        ("htop", "pacman") => "htop",
        ("htop", "apk") => "htop",
        // Default: pass through as-is
        _ => generic,
    }
}

/// 安装软件（跨平台，多包管理器支持）
#[tauri::command]
pub async fn install_software(package_name: String) -> Result<String, String> {
    let managers = detect_package_managers();

    if managers.is_empty() {
        return Err("No supported package manager found on this system".to_string());
    }

    // 克隆到 spawn_blocking 闭包中
    let managers_clone: Vec<_> = managers.iter().map(|pm| PackageManager {
        name: pm.name,
        binary: pm.binary,
        needs_sudo: pm.needs_sudo,
        install_args: pm.install_args,
        uninstall_args: pm.uninstall_args,
    }).collect();
    let pkg_name = package_name.clone();

    tokio::task::spawn_blocking(move || {
        let mut last_error = String::new();

        for pm in &managers_clone {
            let pkg = map_package_name(&pkg_name, pm.name);

            let mut cmd = if pm.needs_sudo {
                let mut c = Command::new("sudo");
                c.arg(pm.binary);
                c
            } else {
                Command::new(pm.binary)
            };

            cmd.args(pm.install_args).arg(pkg);

            match cmd.output() {
                Ok(output) => {
                    if output.status.success() {
                        return Ok(format!(
                            "Successfully installed {} via {}",
                            pkg_name, pm.name
                        ));
                    }
                    last_error = String::from_utf8_lossy(&output.stderr).to_string();
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }
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

/// 卸载软件（跨平台，多包管理器支持）
#[tauri::command]
pub async fn uninstall_software(package_name: String) -> Result<String, String> {
    let managers = detect_package_managers();

    if managers.is_empty() {
        return Err("No supported package manager found on this system".to_string());
    }

    let managers_clone: Vec<_> = managers.iter().map(|pm| PackageManager {
        name: pm.name,
        binary: pm.binary,
        needs_sudo: pm.needs_sudo,
        install_args: pm.install_args,
        uninstall_args: pm.uninstall_args,
    }).collect();
    let pkg_name = package_name.clone();

    tokio::task::spawn_blocking(move || {
        let mut last_error = String::new();

        for pm in &managers_clone {
            let pkg = map_package_name(&pkg_name, pm.name);

            let mut cmd = if pm.needs_sudo {
                let mut c = Command::new("sudo");
                c.arg(pm.binary);
                c
            } else {
                Command::new(pm.binary)
            };

            cmd.args(pm.uninstall_args).arg(pkg);

            match cmd.output() {
                Ok(output) => {
                    if output.status.success() {
                        return Ok(format!(
                            "Successfully uninstalled {} via {}",
                            pkg_name, pm.name
                        ));
                    }
                    last_error = String::from_utf8_lossy(&output.stderr).to_string();
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }
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
