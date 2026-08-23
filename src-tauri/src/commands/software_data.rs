/// 静态软件数据（从 software.rs 拆分出来）：
/// - get_download_url：各软件官方下载 URL 的静态生成规则
/// - GUI_APPS：GUI 应用名单（跳过 --version 检测）
/// - SoftwareDef / build_software_defs：软件定义表
/// - map_package_name：通用包名 -> 各包管理器实际包名映射
///
/// 生成当前平台的下载 URL
pub(super) fn get_download_url(name: &str, version: &str) -> Option<String> {
    match name {
        "Visual Studio Code" => Some(format!(
            "https://code.visualstudio.com/sha/download?build=stable&os={}",
            if cfg!(target_os = "linux") {
                "linux-x64"
            } else if cfg!(target_os = "macos") {
                if cfg!(target_arch = "aarch64") {
                    "darwin-arm64"
                } else {
                    "darwin-x64"
                }
            } else {
                "win32-x64"
            }
        )),
        "Node.js" => {
            let os = if cfg!(target_os = "linux") {
                "linux"
            } else if cfg!(target_os = "macos") {
                "darwin"
            } else {
                "win"
            };
            let arch = if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "x64"
            };
            let ext = if cfg!(target_os = "windows") {
                "zip"
            } else if cfg!(target_os = "macos") {
                "tar.gz"
            } else {
                "tar.xz"
            };
            Some(format!(
                "https://nodejs.org/dist/v{version}/node-v{version}-{os}-{arch}.{ext}"
            ))
        }
        "Go" => {
            let os = if cfg!(target_os = "linux") {
                "linux"
            } else if cfg!(target_os = "macos") {
                "darwin"
            } else {
                "windows"
            };
            let arch = if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "amd64"
            };
            let ext = if cfg!(target_os = "windows") {
                "zip"
            } else {
                "tar.gz"
            };
            Some(format!("https://go.dev/dl/go{version}.{os}-{arch}.{ext}"))
        }
        "Neovim" => {
            let os = if cfg!(target_os = "linux") {
                if cfg!(target_arch = "aarch64") {
                    "linux-arm64"
                } else {
                    "linux64"
                }
            } else if cfg!(target_os = "macos") {
                if cfg!(target_arch = "aarch64") {
                    "macos-arm64"
                } else {
                    "macos-x86_64"
                }
            } else {
                "win64"
            };
            let ext = if cfg!(target_os = "windows") {
                "zip"
            } else {
                "tar.gz"
            };
            Some(format!(
                "https://github.com/neovim/neovim/releases/download/stable/nvim-{os}.{ext}"
            ))
        }
        "Git" => {
            if cfg!(target_os = "windows") {
                Some(format!("https://github.com/git-for-windows/git/releases/download/v{version}.windows.1/Git-{version}-64-bit.exe"))
            } else {
                Some(format!(
                    "https://github.com/git/git/archive/refs/tags/v{version}.tar.gz"
                ))
            }
        }
        _ => None,
    }
}

/// GUI 应用名单：这些程序不支持 --version，执行会直接启动 GUI，跳过版本检测
/// 注意: code --version 和 docker --version 可正常返回版本信息，不在此列
pub(super) const GUI_APPS: &[&str] = &[
    "postman",
    "dbeaver",
    "dbeaver-ce",
    "mysql-workbench",
    "gparted",
];

pub(super) struct SoftwareDef {
    pub name: &'static str,
    pub cmd: &'static str,
    pub category: &'static str,
    pub package_name: &'static str,
}

pub(super) fn build_software_defs() -> Vec<SoftwareDef> {
    let mut defs = Vec::with_capacity(24);

    // ============ IDEs & Editors ============
    defs.push(SoftwareDef {
        name: "Visual Studio Code",
        cmd: "code",
        category: "ide",
        package_name: "code",
    });
    defs.push(SoftwareDef {
        name: "Neovim",
        cmd: "nvim",
        category: "ide",
        package_name: "neovim",
    });
    defs.push(SoftwareDef {
        name: "Vim",
        cmd: "vim",
        category: "ide",
        package_name: "vim",
    });
    defs.push(SoftwareDef {
        name: "Sublime Text",
        cmd: "subl",
        category: "ide",
        package_name: "sublime-text",
    });
    defs.push(SoftwareDef {
        name: "Zed",
        cmd: "zed",
        category: "ide",
        package_name: "zed",
    });
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        defs.push(SoftwareDef {
            name: "Postman",
            cmd: "postman",
            category: "ide",
            package_name: "postman",
        });
        defs.push(SoftwareDef {
            name: "IntelliJ IDEA Community",
            cmd: "idea",
            category: "ide",
            package_name: "intellij-idea-community",
        });
    }

    // ============ Databases ============
    defs.push(SoftwareDef {
        name: "DBeaver Community",
        cmd: "dbeaver",
        category: "database",
        package_name: "dbeaver-ce",
    });
    defs.push(SoftwareDef {
        name: "SQLite",
        cmd: "sqlite3",
        category: "database",
        package_name: "sqlite",
    });
    defs.push(SoftwareDef {
        name: "PostgreSQL Client",
        cmd: "psql",
        category: "database",
        package_name: "postgresql-client",
    });
    defs.push(SoftwareDef {
        name: "Redis",
        cmd: "redis-cli",
        category: "database",
        package_name: "redis",
    });
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        defs.push(SoftwareDef {
            name: "MySQL Workbench",
            cmd: "mysql-workbench",
            category: "database",
            package_name: "mysql-workbench",
        });
        defs.push(SoftwareDef {
            name: "TablePlus",
            cmd: "tableplus",
            category: "database",
            package_name: "tableplus",
        });
    }

    // ============ CLI Tools ============
    defs.push(SoftwareDef {
        name: "Git",
        cmd: "git",
        category: "cli",
        package_name: "git",
    });
    defs.push(SoftwareDef {
        name: "curl",
        cmd: "curl",
        category: "cli",
        package_name: "curl",
    });
    defs.push(SoftwareDef {
        name: "wget",
        cmd: "wget",
        category: "cli",
        package_name: "wget",
    });
    defs.push(SoftwareDef {
        name: "OpenSSH Client",
        cmd: "ssh",
        category: "cli",
        package_name: "openssh-client",
    });
    defs.push(SoftwareDef {
        name: "GCC",
        cmd: "gcc",
        category: "cli",
        package_name: "gcc",
    });
    defs.push(SoftwareDef {
        name: "Clang",
        cmd: "clang",
        category: "cli",
        package_name: "clang",
    });
    defs.push(SoftwareDef {
        name: "CMake",
        cmd: "cmake",
        category: "cli",
        package_name: "cmake",
    });
    defs.push(SoftwareDef {
        name: "htop",
        cmd: "htop",
        category: "cli",
        package_name: "htop",
    });
    defs.push(SoftwareDef {
        name: "tmux",
        cmd: "tmux",
        category: "cli",
        package_name: "tmux",
    });
    defs.push(SoftwareDef {
        name: "ripgrep",
        cmd: "rg",
        category: "cli",
        package_name: "ripgrep",
    });
    defs.push(SoftwareDef {
        name: "fd",
        cmd: "fd",
        category: "cli",
        package_name: "fd-find",
    });
    defs.push(SoftwareDef {
        name: "jq",
        cmd: "jq",
        category: "cli",
        package_name: "jq",
    });
    defs.push(SoftwareDef {
        name: "fzf",
        cmd: "fzf",
        category: "cli",
        package_name: "fzf",
    });
    #[cfg(target_os = "linux")]
    {
        defs.push(SoftwareDef {
            name: "GParted",
            cmd: "gparted",
            category: "cli",
            package_name: "gparted",
        });
    }

    // ============ Runtimes & Package Managers ============
    defs.push(SoftwareDef {
        name: "Node.js",
        cmd: "node",
        category: "runtime",
        package_name: "nodejs",
    });
    defs.push(SoftwareDef {
        name: "Python 3",
        cmd: "python3",
        category: "runtime",
        package_name: "python3",
    });
    defs.push(SoftwareDef {
        name: "Go",
        cmd: "go",
        category: "runtime",
        package_name: "golang",
    });
    defs.push(SoftwareDef {
        name: "Rust",
        cmd: "rustc",
        category: "runtime",
        package_name: "rust",
    });
    defs.push(SoftwareDef {
        name: "Ruby",
        cmd: "ruby",
        category: "runtime",
        package_name: "ruby",
    });
    defs.push(SoftwareDef {
        name: "Java (JDK)",
        cmd: "java",
        category: "runtime",
        package_name: "openjdk-17-jdk",
    });
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        defs.push(SoftwareDef {
            name: "Docker Desktop",
            cmd: "docker",
            category: "runtime",
            package_name: "docker-desktop",
        });
    }
    #[cfg(target_os = "linux")]
    {
        defs.push(SoftwareDef {
            name: "Docker Engine",
            cmd: "docker",
            category: "runtime",
            package_name: "docker-ce",
        });
    }

    defs
}

pub(super) fn map_package_name<'a>(generic: &'a str, pm_name: &str) -> &'a str {
    match (generic, pm_name) {
        // VS Code
        ("code", "winget") => "Microsoft.VisualStudioCode",
        ("code", "chocolatey") => "vscode",
        ("code", "brew") => "visual-studio-code",
        ("code", "apt" | "dnf" | "zypper") => "code",
        // Neovim
        ("neovim", "brew") => "neovim",
        ("neovim", "winget") => "Neovim.Neovim",
        ("neovim", "apt" | "pacman" | "zypper") => "neovim",
        // Node.js
        ("nodejs", "apt" | "pacman" | "dnf") => "nodejs",
        ("nodejs", "zypper") => "nodejs20",
        ("nodejs", "brew") => "node",
        ("nodejs", "winget") => "OpenJS.NodeJS.LTS",
        // Python
        ("python3", "apt" | "pacman" | "zypper") => "python3",
        ("python3", "brew") => "python",
        ("python3", "winget") => "Python.Python.3.12",
        // Go
        ("golang", "apt" | "dnf" | "zypper") => "golang",
        ("golang", "brew") => "go",
        ("golang", "winget") => "GoLang.Go",
        ("golang", "pacman") => "go",
        // Rust
        ("rust", "brew") => "rustup",
        ("rust", "winget") => "Rustlang.Rustup",
        ("rust", "pacman" | "zypper") => "rust",
        ("rust", "apt") => "rustc",
        // Ruby
        ("ruby", "apt") => "ruby-full",
        ("ruby", "brew" | "zypper") => "ruby",
        // Java
        ("openjdk-17-jdk", "apt") => "openjdk-17-jdk",
        ("openjdk-17-jdk", "brew") => "openjdk@17",
        ("openjdk-17-jdk", "winget") => "Microsoft.OpenJDK.17",
        ("openjdk-17-jdk", "zypper") => "java-17-openjdk",
        // Docker
        ("docker-ce", "apt" | "dnf") => "docker-ce",
        ("docker-ce", "pacman" | "zypper") => "docker",
        // Git
        ("git", "apt" | "brew" | "pacman" | "zypper") => "git",
        ("git", "winget") => "Git.Git",
        // curl
        ("curl", "apt" | "brew" | "zypper") => "curl",
        ("curl", "winget") => "cURL.cURL",
        // wget
        ("wget", "apt" | "brew" | "zypper") => "wget",
        ("wget", "winget") => "GNU.Wget",
        // OpenSSH
        ("openssh-client", "apt") => "openssh-client",
        ("openssh-client", "brew" | "zypper") => "openssh",
        // GCC
        ("gcc", "apt" | "brew" | "pacman" | "zypper") => "gcc",
        // Clang
        ("clang", "apt" | "pacman" | "zypper") => "clang",
        ("clang", "brew") => "llvm",
        // CMake
        ("cmake", "apt" | "brew" | "pacman" | "zypper") => "cmake",
        ("cmake", "winget") => "Kitware.CMake",
        // ripgrep
        ("ripgrep", "apt" | "brew" | "pacman" | "zypper") => "ripgrep",
        ("ripgrep", "winget") => "BurntSushi.ripgrep.MSVC",
        // fd
        ("fd-find", "apt") => "fd-find",
        ("fd-find", "brew" | "pacman" | "zypper") => "fd",
        ("fd-find", "winget") => "sharkdp.fd",
        // jq
        ("jq", "apt" | "brew" | "pacman" | "zypper") => "jq",
        ("jq", "winget") => "jqlang.jq",
        // fzf
        ("fzf", "apt" | "brew" | "pacman" | "zypper") => "fzf",
        ("fzf", "winget") => "junegunn.fzf",
        // htop
        ("htop", "apt" | "brew" | "pacman" | "zypper") => "htop",
        // tmux
        ("tmux", "apt" | "brew" | "pacman" | "zypper") => "tmux",
        // Redis
        ("redis", "apt") => "redis-server",
        ("redis", "brew" | "pacman" | "zypper") => "redis",
        // SQLite
        ("sqlite", "apt" | "zypper") => "sqlite3",
        ("sqlite", "brew") => "sqlite",
        ("sqlite", "winget") => "SQLite.SQLite",
        // PostgreSQL
        ("postgresql-client", "apt") => "postgresql-client",
        ("postgresql-client", "brew") => "libpq",
        ("postgresql-client", "pacman") => "postgresql-libs",
        ("postgresql-client", "zypper") => "postgresql16",
        // Sublime Text
        ("sublime-text", "brew") => "sublime-text",
        ("sublime-text", "apt" | "zypper") => "sublime-text",
        ("sublime-text", "winget") => "SublimeHQ.SublimeText.4",
        // Zed
        ("zed", "brew") => "zed",
        ("zed", "winget") => "Zed.Zed",
        // GParted
        ("gparted", "apt" | "pacman" | "zypper") => "gparted",
        // DBeaver
        ("dbeaver-ce", "brew") => "dbeaver-community",
        ("dbeaver-ce", "winget") => "dbeaver.dbeaver",
        ("dbeaver-ce", "apt" | "zypper") => "dbeaver-ce",
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
        // Vim
        ("vim", "winget") => "vim.vim",
        ("vim", "chocolatey") => "vim",
        // GParted (no Windows port, keep Linux mappings above)
        // Flatpak IDs — when PM is flatpak, generic name must map to app ID
        ("code", "flatpak") => "com.visualstudio.code",
        ("neovim", "flatpak") => "io.neovim.nvim",
        ("dbeaver-ce", "flatpak") => "io.dbeaver.DBeaverCommunity",
        ("postman", "flatpak") => "com.getpostman.Postman",
        ("sublime-text", "flatpak") => "com.sublimetext.three",
        ("gimp", "flatpak") => "org.gimp.GIMP",
        ("vlc", "flatpak") => "org.videolan.VLC",
        // Snap names
        ("code", "snap") => "code",
        ("neovim", "snap") => "nvim",
        ("postman", "snap") => "postman",
        ("sublime-text", "snap") => "sublime-text",
        // Chocolatey fallbacks for CLI tools absent from winget
        ("neovim", "chocolatey") => "neovim",
        ("git", "chocolatey") => "git",
        ("curl", "chocolatey") => "curl",
        ("wget", "chocolatey") => "wget",
        ("python3", "chocolatey") => "python",
        ("golang", "chocolatey") => "golang",
        ("nodejs", "chocolatey") => "nodejs-lts",
        ("rust", "chocolatey") => "rustup.install",
        ("sqlite", "chocolatey") => "sqlite",
        ("jq", "chocolatey") => "jq",
        ("ripgrep", "chocolatey") => "ripgrep",
        ("fd-find", "chocolatey") => "fd",
        ("fzf", "chocolatey") => "fzf",
        ("cmake", "chocolatey") => "cmake",
        ("htop", "chocolatey") => "htop",
        ("redis", "chocolatey") => "redis-64",
        ("postgresql-client", "chocolatey") => "postgresql",
        ("gcc", "chocolatey") => "mingw",
        ("clang", "chocolatey") => "llvm",
        ("vim", "flatpak") => "org.vim.Vim",
        ("firefox", "flatpak") => "org.mozilla.firefox",
        ("chromium", "flatpak") => "org.chromium.Chromium",
        // Default: 直接返回通用名
        _ => generic,
    }
}
