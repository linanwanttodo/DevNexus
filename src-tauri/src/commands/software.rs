use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

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

/// 版本来源：从哪里获取可用版本列表
#[derive(Clone)]
enum VersionSource {
    GitHubReleases { owner: &'static str, repo: &'static str },
    NodeDist,
    GoDev,
}

/// 获取软件版本来源
fn get_version_source(name: &str) -> Option<VersionSource> {
    match name {
        "Visual Studio Code" => Some(VersionSource::GitHubReleases { owner: "microsoft", repo: "vscode" }),
        "Neovim" => Some(VersionSource::GitHubReleases { owner: "neovim", repo: "neovim" }),
        "Node.js" => Some(VersionSource::NodeDist),
        "Go" => Some(VersionSource::GoDev),
        "Python 3" => Some(VersionSource::GitHubReleases { owner: "python", repo: "cpython" }),
        "Git" => Some(VersionSource::GitHubReleases { owner: "git", repo: "git" }),
        "Rust" => Some(VersionSource::GitHubReleases { owner: "rust-lang", repo: "rust" }),
        _ => None,
    }
}

/// 生成当前平台的下载 URL
fn get_download_url(name: &str, version: &str) -> Option<String> {
    match name {
        "Visual Studio Code" => Some(format!(
            "https://code.visualstudio.com/sha/download?build=stable&os={}",
            if cfg!(target_os = "linux") { "linux-x64" }
            else if cfg!(target_os = "macos") { { if cfg!(target_arch = "aarch64") { "darwin-arm64" } else { "darwin-x64" } } }
            else { "win32-x64" }
        )),
        "Node.js" => {
            let os = if cfg!(target_os = "linux") { "linux" } else if cfg!(target_os = "macos") { "darwin" } else { "win" };
            let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
            let ext = if cfg!(target_os = "windows") { "zip" } else if cfg!(target_os = "macos") { "tar.gz" } else { "tar.xz" };
            Some(format!("https://nodejs.org/dist/v{version}/node-v{version}-{os}-{arch}.{ext}"))
        }
        "Go" => {
            let os = if cfg!(target_os = "linux") { "linux" } else if cfg!(target_os = "macos") { "darwin" } else { "windows" };
            let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "amd64" };
            let ext = if cfg!(target_os = "windows") { "zip" } else { "tar.gz" };
            Some(format!("https://go.dev/dl/go{version}.{os}-{arch}.{ext}"))
        }
        "Neovim" => {
            #[cfg(target_os = "linux")]
            { Some(format!("https://github.com/neovim/neovim/releases/download/stable/nvim-linux64.tar.gz")) }
            #[cfg(target_os = "macos")]
            { Some(format!("https://github.com/neovim/neovim/releases/download/stable/nvim-macos-arm64.tar.gz")) }
            #[cfg(target_os = "windows")]
            { Some(format!("https://github.com/neovim/neovim/releases/download/stable/nvim-win64.zip")) }
        }
        "Git" => {
            #[cfg(target_os = "linux")]
            { Some(format!("https://github.com/git/git/archive/refs/tags/v{version}.tar.gz")) }
            #[cfg(target_os = "macos")]
            { Some(format!("https://github.com/git/git/archive/refs/tags/v{version}.tar.gz")) }
            #[cfg(target_os = "windows")]
            { Some(format!("https://github.com/git-for-windows/git/releases/download/v{version}.windows.1/Git-{version}-64-bit.exe")) }
        }
        _ => None,
    }
}

/// GUI 应用名单：这些程序不支持 --version，执行会直接启动 GUI，跳过版本检测
/// 注意: code --version 和 docker --version 可正常返回版本信息，不在此列
const GUI_APPS: &[&str] = &[
    "postman", "dbeaver", "dbeaver-ce", "mysql-workbench", "gparted",
];

/// 安全获取软件版本：对 GUI 应用跳过，避免启动它们
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
        _ => "timeout".to_string(),
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

struct SoftwareDef {
    name: &'static str,
    cmd: &'static str,
    category: &'static str,
    package_name: &'static str,
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
            let found = which::which(cmd).is_ok() || check_common_paths(cmd);
            let version = if found {
                safe_get_version(cmd).await
            } else {
                "N/A".to_string()
            };
            let status = if found { "installed" } else { "available" };
            let action = if found { "Open" } else { "Install" };
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

fn build_software_defs() -> Vec<SoftwareDef> {
    let mut defs = Vec::with_capacity(24);

    // ============ IDEs & Editors ============
    defs.push(SoftwareDef { name: "Visual Studio Code", cmd: "code", category: "ide", package_name: "code" });
    defs.push(SoftwareDef { name: "Neovim", cmd: "nvim", category: "ide", package_name: "neovim" });
    defs.push(SoftwareDef { name: "Vim", cmd: "vim", category: "ide", package_name: "vim" });
    defs.push(SoftwareDef { name: "Sublime Text", cmd: "subl", category: "ide", package_name: "sublime-text" });
    defs.push(SoftwareDef { name: "Zed", cmd: "zed", category: "ide", package_name: "zed" });
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        defs.push(SoftwareDef { name: "Postman", cmd: "postman", category: "ide", package_name: "postman" });
        defs.push(SoftwareDef { name: "IntelliJ IDEA Community", cmd: "idea", category: "ide", package_name: "intellij-idea-community" });
    }

    // ============ Databases ============
    defs.push(SoftwareDef { name: "DBeaver Community", cmd: "dbeaver", category: "database", package_name: "dbeaver-ce" });
    defs.push(SoftwareDef { name: "SQLite", cmd: "sqlite3", category: "database", package_name: "sqlite" });
    defs.push(SoftwareDef { name: "PostgreSQL Client", cmd: "psql", category: "database", package_name: "postgresql-client" });
    defs.push(SoftwareDef { name: "Redis", cmd: "redis-cli", category: "database", package_name: "redis" });
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        defs.push(SoftwareDef { name: "MySQL Workbench", cmd: "mysql-workbench", category: "database", package_name: "mysql-workbench" });
        defs.push(SoftwareDef { name: "TablePlus", cmd: "tableplus", category: "database", package_name: "tableplus" });
    }

    // ============ CLI Tools ============
    defs.push(SoftwareDef { name: "Git", cmd: "git", category: "cli", package_name: "git" });
    defs.push(SoftwareDef { name: "curl", cmd: "curl", category: "cli", package_name: "curl" });
    defs.push(SoftwareDef { name: "wget", cmd: "wget", category: "cli", package_name: "wget" });
    defs.push(SoftwareDef { name: "OpenSSH Client", cmd: "ssh", category: "cli", package_name: "openssh-client" });
    defs.push(SoftwareDef { name: "GCC", cmd: "gcc", category: "cli", package_name: "gcc" });
    defs.push(SoftwareDef { name: "Clang", cmd: "clang", category: "cli", package_name: "clang" });
    defs.push(SoftwareDef { name: "CMake", cmd: "cmake", category: "cli", package_name: "cmake" });
    defs.push(SoftwareDef { name: "htop", cmd: "htop", category: "cli", package_name: "htop" });
    defs.push(SoftwareDef { name: "tmux", cmd: "tmux", category: "cli", package_name: "tmux" });
    defs.push(SoftwareDef { name: "ripgrep", cmd: "rg", category: "cli", package_name: "ripgrep" });
    defs.push(SoftwareDef { name: "fd", cmd: "fd", category: "cli", package_name: "fd-find" });
    defs.push(SoftwareDef { name: "jq", cmd: "jq", category: "cli", package_name: "jq" });
    defs.push(SoftwareDef { name: "fzf", cmd: "fzf", category: "cli", package_name: "fzf" });
    #[cfg(target_os = "linux")]
    {
        defs.push(SoftwareDef { name: "GParted", cmd: "gparted", category: "cli", package_name: "gparted" });
    }

    // ============ Runtimes & Package Managers ============
    defs.push(SoftwareDef { name: "Node.js", cmd: "node", category: "runtime", package_name: "nodejs" });
    defs.push(SoftwareDef { name: "Python 3", cmd: "python3", category: "runtime", package_name: "python3" });
    defs.push(SoftwareDef { name: "Go", cmd: "go", category: "runtime", package_name: "golang" });
    defs.push(SoftwareDef { name: "Rust", cmd: "rustc", category: "runtime", package_name: "rust" });
    defs.push(SoftwareDef { name: "Ruby", cmd: "ruby", category: "runtime", package_name: "ruby" });
    defs.push(SoftwareDef { name: "Java (JDK)", cmd: "java", category: "runtime", package_name: "openjdk-17-jdk" });
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        defs.push(SoftwareDef { name: "Docker Desktop", cmd: "docker", category: "runtime", package_name: "docker-desktop" });
    }
    #[cfg(target_os = "linux")]
    {
        defs.push(SoftwareDef { name: "Docker Engine", cmd: "docker", category: "runtime", package_name: "docker-ce" });
    }

    defs
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

#[cfg(target_os = "windows")]
fn run_elevated(binary: &str, args: &[&str]) -> Result<std::process::Output, String> {
    // Windows 包管理器 (winget, choco) 的 needs_sudo 均为 false，
    // 此函数仅用于编译通过，实际不会被调用。
    std::process::Command::new(binary)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", binary, e))
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

/// 获取软件残留配置/缓存/数据目录的路径列表
///
/// 对已知的开发者工具返回精确的路径，否则基于通用模式生成候选路径。
fn get_cleanup_paths(app_name: &str, package_name: &str) -> Vec<PathBuf> {
let home = std::env::var("HOME").unwrap_or_default();
let mut paths: Vec<PathBuf> = Vec::new();

// ——— 已知工具的精确路径映射（不区分大小写匹配） ———
let app_name_lower = app_name.to_lowercase();
let pkg_lower = package_name.to_lowercase();

match app_name_lower.as_str() {
    // Node.js
    "node.js" | "nodejs" => {
        paths.push(PathBuf::from(&home).join(".npm"));
        paths.push(PathBuf::from(&home).join(".node-gyp"));
        #[cfg(unix)]
        {
            paths.push(PathBuf::from(&home).join(".config/configstore"));
            paths.push(PathBuf::from("/usr/local/lib/node_modules"));
        }
        #[cfg(target_os = "macos")]
        {
            paths.push(PathBuf::from(&home).join("Library/Preferences/node"));
            paths.push(PathBuf::from(&home).join("Library/Caches/node"));
        }
        #[cfg(windows)]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                paths.push(PathBuf::from(appdata).join("npm"));
                paths.push(PathBuf::from(appdata).join("npm-cache"));
            }
        }
    }
    // Python
    "python 3" | "python3" | "python" => {
        #[cfg(unix)]
        {
            paths.push(PathBuf::from(&home).join(".local/lib/python*"));
            paths.push(PathBuf::from(&home).join(".cache/pip"));
            paths.push(PathBuf::from(&home).join(".config/pip"));
            paths.push(PathBuf::from(&home).join(".python_history"));
        }
        #[cfg(target_os = "macos")]
        {
            paths.push(PathBuf::from(&home).join("Library/Caches/pip"));
            paths.push(PathBuf::from(&home).join("Library/Python"));
        }
        #[cfg(windows)]
        {
            if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
                paths.push(PathBuf::from(localappdata).join("pip"));
                paths.push(PathBuf::from(localappdata).join("Python"));
            }
        }
    }
    // Rust
    "rust" | "rustc" => {
        #[cfg(unix)]
        {
            paths.push(PathBuf::from(&home).join(".rustup"));
            paths.push(PathBuf::from(&home).join(".cargo"));
        }
        #[cfg(windows)]
        {
            if let Ok(userprofile) = std::env::var("USERPROFILE") {
                paths.push(PathBuf::from(userprofile).join(".rustup"));
                paths.push(PathBuf::from(userprofile).join(".cargo"));
            }
        }
    }
    // Go
    "go" | "golang" => {
        #[cfg(unix)]
        {
            paths.push(PathBuf::from(&home).join("go"));
            paths.push(PathBuf::from(&home).join(".cache/go"));
        }
        #[cfg(windows)]
        {
            if let Ok(userprofile) = std::env::var("USERPROFILE") {
                paths.push(PathBuf::from(userprofile).join("go"));
            }
        }
    }
    // VS Code
    "visual studio code" | "code" => {
        #[cfg(target_os = "linux")]
        {
            paths.push(PathBuf::from(&home).join(".config/Code"));
            paths.push(PathBuf::from(&home).join(".vscode"));
        }
        #[cfg(target_os = "macos")]
        {
            paths.push(PathBuf::from(&home).join("Library/Application Support/Code"));
            paths.push(PathBuf::from(&home).join(".vscode"));
        }
        #[cfg(windows)]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                paths.push(PathBuf::from(appdata).join("Code"));
            }
            if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
                paths.push(PathBuf::from(localappdata).join("Programs/Microsoft VS Code"));
            }
        }
    }
    // Firefox
    "firefox" => {
        #[cfg(target_os = "linux")]
        {
            paths.push(PathBuf::from(&home).join(".mozilla/firefox"));
        }
        #[cfg(target_os = "macos")]
        {
            paths.push(PathBuf::from(&home).join("Library/Application Support/Firefox"));
            paths.push(PathBuf::from(&home).join("Library/Caches/Firefox"));
        }
        #[cfg(windows)]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                paths.push(PathBuf::from(appdata).join("Mozilla/Firefox"));
            }
            if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
                paths.push(PathBuf::from(localappdata).join("Mozilla/Firefox"));
            }
        }
    }
    // Chrome / Chromium
    "chrome" | "google chrome" | "chromium" | "chromium-browser" => {
        #[cfg(target_os = "linux")]
        {
            paths.push(PathBuf::from(&home).join(".config/google-chrome"));
            paths.push(PathBuf::from(&home).join(".cache/google-chrome"));
            paths.push(PathBuf::from(&home).join(".config/chromium"));
            paths.push(PathBuf::from(&home).join(".cache/chromium"));
        }
        #[cfg(target_os = "macos")]
        {
            paths.push(PathBuf::from(&home).join("Library/Application Support/Google/Chrome"));
            paths.push(PathBuf::from(&home).join("Library/Caches/Google/Chrome"));
        }
        #[cfg(windows)]
        {
            if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
                paths.push(PathBuf::from(localappdata).join("Google/Chrome"));
                paths.push(PathBuf::from(localappdata).join("Google/Chrome/User Data"));
            }
        }
    }
    // Docker
    "docker" | "docker desktop" | "docker engine" => {
        #[cfg(target_os = "linux")]
        {
            paths.push(PathBuf::from(&home).join(".docker"));
            paths.push(PathBuf::from("/var/lib/docker"));
        }
        #[cfg(target_os = "macos")]
        {
            paths.push(PathBuf::from(&home).join("Library/Containers/com.docker.docker"));
            paths.push(PathBuf::from(&home).join("Library/Application Support/Docker"));
            paths.push(PathBuf::from(&home).join(".docker"));
        }
        #[cfg(windows)]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                paths.push(PathBuf::from(appdata).join("Docker"));
            }
            if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
                paths.push(PathBuf::from(localappdata).join("Docker"));
            }
        }
    }
    // ——— 未识别的软件：用通用模式 ———
    _ => {
        let name_slug = app_name_lower.replace([' ', '_'], "-");

        #[cfg(unix)]
        {
            // ~/.config/<name>
            paths.push(PathBuf::from(&home).join(format!(".config/{}", name_slug)));
            paths.push(PathBuf::from(&home).join(format!(".config/{}", pkg_lower)));
            // ~/.cache/<name>
            paths.push(PathBuf::from(&home).join(format!(".cache/{}", name_slug)));
            paths.push(PathBuf::from(&home).join(format!(".cache/{}", pkg_lower)));
            // ~/.local/share/<name>
            paths.push(PathBuf::from(&home).join(format!(".local/share/{}", name_slug)));
            paths.push(PathBuf::from(&home).join(format!(".local/share/{}", pkg_lower)));
            // ~/.<name>
            paths.push(PathBuf::from(&home).join(format!(".{}", name_slug)));
            paths.push(PathBuf::from(&home).join(format!(".{}", pkg_lower)));
        }

        #[cfg(target_os = "macos")]
        {
            // ~/Library/Application Support/<name>
            paths.push(PathBuf::from(&home).join(format!("Library/Application Support/{}", name_slug)));
            paths.push(PathBuf::from(&home).join(format!("Library/Application Support/{}", pkg_lower)));
            // ~/Library/Caches/<name>
            paths.push(PathBuf::from(&home).join(format!("Library/Caches/{}", name_slug)));
            paths.push(PathBuf::from(&home).join(format!("Library/Caches/{}", pkg_lower)));
            // ~/Library/Preferences/<name>
            paths.push(PathBuf::from(&home).join(format!("Library/Preferences/{}", name_slug)));
            paths.push(PathBuf::from(&home).join(format!("Library/Preferences/{}", pkg_lower)));
        }

        #[cfg(windows)]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                paths.push(PathBuf::from(appdata).join(&name_slug));
                paths.push(PathBuf::from(appdata).join(&pkg_lower));
            }
            if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
                paths.push(PathBuf::from(localappdata).join(&name_slug));
                paths.push(PathBuf::from(localappdata).join(&pkg_lower));
            }
        }
    }
}

// 去重保留顺序
let mut seen = std::collections::HashSet::new();
paths.into_iter().filter(|p| seen.insert(p.clone())).collect()
}

/// 深度卸载：先执行标准卸载，再清理残留的配置文件、缓存和数据目录
#[tauri::command]
pub async fn uninstall_software_deep(package_name: String, app_name: String) -> Result<String, String> {
// (a) 先执行标准卸载
let result = uninstall_software(package_name.clone()).await?;

// (b) 获取所有可能的清理路径
let cleanup_paths = get_cleanup_paths(&app_name, &package_name);

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

// (d) 构造结果消息
let mut message = result;
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

// 至少清理了一些内容才算成功，但即使全部失败也返回 Ok（让上层决定）
Ok(message)
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

// ==================== 版本查询 & 直接下载安装 ====================

/// 从 GitHub Releases API 获取版本列表
async fn fetch_github_versions(owner: &str, repo: &str) -> Result<Vec<String>, String> {
    let url = format!("https://api.github.com/repos/{}/{}/releases?per_page=30", owner, repo);
    let client = reqwest::Client::new();
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
        .filter(|v| !v.contains("rc") && !v.contains("beta") && !v.contains("alpha") && !v.contains("nightly"))
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
    let resp = reqwest::get(url)
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
    let resp = reqwest::get(url)
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

/// 从官方源下载并安装指定版本的软件
#[tauri::command]
pub async fn install_software_from_url(package_name: String, version: String) -> Result<String, String> {
    let defs = build_software_defs();
    let def = defs
        .iter()
        .find(|d| d.package_name == package_name || d.name == package_name)
        .ok_or_else(|| format!("Unknown software: {}", package_name))?;

    let url = get_download_url(def.name, &version)
        .ok_or_else(|| format!("No download URL configured for {}", def.name))?;

    let install_dir = get_install_base_dir().join(&package_name).join(&version);
    if install_dir.exists() {
        return Err(format!("Version {} of {} is already installed at {}", version, def.name, install_dir.display()));
    }

    let temp_dir = std::env::temp_dir().join("devnexus-install");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // 下载
    let filename = url.rsplit('/').next().unwrap_or("download");
    let filepath = temp_dir.join(filename);

    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to download: {}", e))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    std::fs::write(&filepath, &bytes)
        .map_err(|e| format!("Failed to save file: {}", e))?;

    // 创建安装目录
    std::fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Failed to create install dir: {}", e))?;

    // 解压
    let filename_lower = filename.to_lowercase();
    if filename_lower.ends_with(".tar.gz") || filename_lower.ends_with(".tgz") {
        let output = Command::new("tar")
            .args(["-xzf", &filepath.to_string_lossy(), "-C", &install_dir.to_string_lossy()])
            .output()
            .map_err(|e| format!("Failed to run tar: {}", e))?;
        if !output.status.success() {
            return Err(format!("Extraction failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
    } else if filename_lower.ends_with(".tar.xz") {
        let output = Command::new("tar")
            .args(["-xJf", &filepath.to_string_lossy(), "-C", &install_dir.to_string_lossy()])
            .output()
            .map_err(|e| format!("Failed to run tar: {}", e))?;
        if !output.status.success() {
            return Err(format!("Extraction failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
    } else if filename_lower.ends_with(".zip") {
        let output = Command::new("unzip")
            .args(["-o", &filepath.to_string_lossy(), "-d", &install_dir.to_string_lossy()])
            .output()
            .map_err(|e| format!("Failed to run unzip: {}", e))?;
        if !output.status.success() {
            // fallback: 用 Rust zip 库解压
            let file = std::fs::File::open(&filepath)
                .map_err(|e| format!("Failed to open zip: {}", e))?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| format!("Failed to read zip: {}", e))?;
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i)
                    .map_err(|e| format!("Failed to read zip entry: {}", e))?;
                let outpath = install_dir.join(entry.name());
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
                .args(["-R", &format!("{}/{}", mount_point, def.name), &install_dir.to_string_lossy()])
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
        std::fs::copy(&filepath, install_dir.join(&filename))
            .map_err(|e| format!("Failed to copy file: {}", e))?;
    }

    // 清理临时文件
    std::fs::remove_file(&filepath).ok();

    // 创建符号链接到 bin 目录
    let bin_dir = get_install_base_dir().parent().unwrap().join("bin");
    std::fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Failed to create bin dir: {}", e))?;

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

    Ok(format!(
        "Successfully installed {} v{}",
        def.name, version
    ))
}
