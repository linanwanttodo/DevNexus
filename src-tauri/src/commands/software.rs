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

    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    let cmd_str = cmd.to_string();
    std::thread::spawn(move || {
        let result = std::process::Command::new(&cmd_str)
            .arg("--version")
            .output();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(std::time::Duration::from_secs(3)) {
        Ok(Ok(output)) if output.status.success() => {
            let ver = String::from_utf8_lossy(&output.stdout);
            let first_line = ver.lines().next().unwrap_or("unknown");
            if first_line.len() > 60 {
                first_line[..57].to_string() + "..."
            } else {
                first_line.to_string()
            }
        }
        _ => "timeout".to_string(),
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

#[derive(Serialize, Clone)]
pub struct PackageManagerInfo {
    pub name: String,
    pub binary: String,
    pub needs_sudo: bool,
}

/// 列出系统上检测到的可用包管理器（前端用于展示引导提示）
#[tauri::command]
pub fn list_package_managers() -> Vec<PackageManagerInfo> {
    let managers = detect_package_managers();
    managers.into_iter().map(|pm| PackageManagerInfo {
        name: pm.name.to_string(),
        binary: pm.binary.to_string(),
        needs_sudo: pm.needs_sudo,
    }).collect()
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
        ("code", "brew") => "visual-studio-code",
        ("code", "apt") => "code",
        ("code", "dnf") => "code",
        // Neovim
        ("neovim", "brew") => "neovim",
        ("neovim", "winget") => "Neovim.Neovim",
        ("neovim", "apt") => "neovim",
        ("neovim", "pacman") => "neovim",
        // Node.js
        ("nodejs", "apt") => "nodejs",
        ("nodejs", "brew") => "node",
        ("nodejs", "winget") => "OpenJS.NodeJS.LTS",
        ("nodejs", "pacman") => "nodejs",
        ("nodejs", "dnf") => "nodejs",
        // Python
        ("python3", "apt") => "python3",
        ("python3", "brew") => "python",
        ("python3", "winget") => "Python.Python.3.12",
        ("python3", "pacman") => "python",
        // Go
        ("golang", "apt") => "golang",
        ("golang", "brew") => "go",
        ("golang", "winget") => "GoLang.Go",
        ("golang", "pacman") => "go",
        ("golang", "dnf") => "golang",
        // Rust
        ("rust", "brew") => "rustup",
        ("rust", "winget") => "Rustlang.Rustup",
        ("rust", "pacman") => "rust",
        ("rust", "apt") => "rustc",
        // Ruby
        ("ruby", "apt") => "ruby-full",
        ("ruby", "brew") => "ruby",
        // Java
        ("openjdk-17-jdk", "apt") => "openjdk-17-jdk",
        ("openjdk-17-jdk", "brew") => "openjdk@17",
        ("openjdk-17-jdk", "winget") => "Microsoft.OpenJDK.17",
        // Docker
        ("docker-ce", "apt") => "docker-ce",
        ("docker-ce", "dnf") => "docker-ce",
        ("docker-ce", "pacman") => "docker",
        // Git
        ("git", "apt") => "git",
        ("git", "brew") => "git",
        ("git", "winget") => "Git.Git",
        ("git", "pacman") => "git",
        // curl
        ("curl", "apt") => "curl",
        ("curl", "brew") => "curl",
        ("curl", "winget") => "cURL.cURL",
        // wget
        ("wget", "apt") => "wget",
        ("wget", "brew") => "wget",
        ("wget", "winget") => "GNU.Wget",
        // OpenSSH
        ("openssh-client", "apt") => "openssh-client",
        ("openssh-client", "brew") => "openssh",
        // GCC
        ("gcc", "apt") => "gcc",
        ("gcc", "brew") => "gcc",
        ("gcc", "pacman") => "gcc",
        // Clang
        ("clang", "apt") => "clang",
        ("clang", "brew") => "llvm",
        ("clang", "pacman") => "clang",
        // CMake
        ("cmake", "apt") => "cmake",
        ("cmake", "brew") => "cmake",
        ("cmake", "winget") => "Kitware.CMake",
        ("cmake", "pacman") => "cmake",
        // ripgrep
        ("ripgrep", "apt") => "ripgrep",
        ("ripgrep", "brew") => "ripgrep",
        ("ripgrep", "winget") => "BurntSushi.ripgrep.MSVC",
        ("ripgrep", "pacman") => "ripgrep",
        // fd
        ("fd-find", "apt") => "fd-find",
        ("fd-find", "brew") => "fd",
        ("fd-find", "winget") => "sharkdp.fd",
        ("fd-find", "pacman") => "fd",
        // jq
        ("jq", "apt") => "jq",
        ("jq", "brew") => "jq",
        ("jq", "winget") => "jqlang.jq",
        ("jq", "pacman") => "jq",
        // fzf
        ("fzf", "apt") => "fzf",
        ("fzf", "brew") => "fzf",
        ("fzf", "winget") => "junegunn.fzf",
        ("fzf", "pacman") => "fzf",
        // htop
        ("htop", "apt") => "htop",
        ("htop", "brew") => "htop",
        ("htop", "pacman") => "htop",
        // tmux
        ("tmux", "apt") => "tmux",
        ("tmux", "brew") => "tmux",
        ("tmux", "pacman") => "tmux",
        // Redis
        ("redis", "apt") => "redis-server",
        ("redis", "brew") => "redis",
        ("redis", "pacman") => "redis",
        // SQLite
        ("sqlite", "apt") => "sqlite3",
        ("sqlite", "brew") => "sqlite",
        ("sqlite", "winget") => "SQLite.SQLite",
        // PostgreSQL
        ("postgresql-client", "apt") => "postgresql-client",
        ("postgresql-client", "brew") => "libpq",
        ("postgresql-client", "pacman") => "postgresql-libs",
        // Sublime Text
        ("sublime-text", "brew") => "sublime-text",
        ("sublime-text", "apt") => "sublime-text",
        ("sublime-text", "winget") => "SublimeHQ.SublimeText.4",
        // Zed
        ("zed", "brew") => "zed",
        ("zed", "winget") => "Zed.Zed",
        // GParted
        ("gparted", "apt") => "gparted",
        ("gparted", "pacman") => "gparted",
        // DBeaver
        ("dbeaver-ce", "brew") => "dbeaver-community",
        ("dbeaver-ce", "winget") => "dbeaver.dbeaver",
        ("dbeaver-ce", "apt") => "dbeaver-ce",
        // Postman (brew cask)
        ("postman", "brew") => "postman",
        ("postman", "winget") => "Postman.Postman",
        // IntelliJ IDEA
        ("intellij-idea-community", "brew") => "intellij-idea-ce",
        ("intellij-idea-community", "winget") => "JetBrains.IntelliJIDEA.Community",
        // MySQL Workbench
        ("mysql-workbench", "brew") => "mysql-workbench",
        ("mysql-workbench", "winget") => "Oracle.MySQLWorkbench",
        // TablePlus
        ("tableplus", "brew") => "tableplus",
        ("tableplus", "winget") => "TablePlus.TablePlus",
        // Docker Desktop
        ("docker-desktop", "brew") => "docker",
        ("docker-desktop", "winget") => "Docker.DockerDesktop",
        // Default: 直接返回通用名
        _ => generic,
    }
}

#[cfg(target_os = "macos")]
fn run_elevated(binary: &str, args: &[&str]) -> Result<std::process::Output, String> {
    let mut cmd_str = binary.to_string();
    for arg in args {
        cmd_str.push(' ');
        cmd_str.push_str(arg);
    }
    let escaped = cmd_str.replace('\\', "\\\\").replace('"', "\\\"");
    std::process::Command::new("osascript")
        .args(["-e", &format!("do shell script \"{}\" with administrator privileges", escaped)])
        .output()
        .map_err(|e| format!("Failed to execute osascript: {}", e))
}

#[cfg(target_os = "linux")]
fn run_elevated(binary: &str, args: &[&str]) -> Result<std::process::Output, String> {
    let mut cmd = std::process::Command::new("pkexec");
    cmd.arg(binary);
    cmd.args(args);
    cmd.output()
        .map_err(|e| format!("Failed to execute pkexec: {}", e))
}

/// 安装软件（跨平台，多包管理器支持）
#[tauri::command]
pub async fn install_software(package_name: String) -> Result<String, String> {
    let managers = detect_package_managers();

    if managers.is_empty() {
        return Err("No supported package manager found on this system.\n\nTo use the Software Center, please install a package manager:\n- macOS: Install Homebrew -> https://brew.sh/\n- Linux: Your distro likely has apt/dnf/pacman/zypper/apk pre-installed\n- Windows: winget comes built-in with Win 11 / Win 10 1809+. Chocolatey: https://chocolatey.org/install".to_string());
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

/// 卸载软件（跨平台，多包管理器支持）
#[tauri::command]
pub async fn uninstall_software(package_name: String) -> Result<String, String> {
    let managers = detect_package_managers();

    if managers.is_empty() {
        return Err("No supported package manager found on this system.\n\nTo use the Software Center, please install a package manager:\n- macOS: Install Homebrew -> https://brew.sh/\n- Linux: Your distro likely has apt/dnf/pacman/zypper/apk pre-installed\n- Windows: winget comes built-in with Win 11 / Win 10 1809+. Chocolatey: https://chocolatey.org/install".to_string());
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
