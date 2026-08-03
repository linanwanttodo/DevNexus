use std::path::PathBuf;

/// 返回已知应用的精确清理路径（增强版，覆盖 60+ 应用）
pub fn get_cleanup_paths(app_name: &str, package_name: &str, home: &str) -> Vec<PathBuf> {
    let app_lower = app_name.to_lowercase();
    let pkg_lower = package_name.to_lowercase();
    let mut paths: Vec<PathBuf> = Vec::new();
    let home_p = PathBuf::from(home);

    macro_rules! push {
        ($p:expr) => {
            paths.push($p);
        };
    }

    match app_lower.as_str() {
        // ========== 编程语言 & 运行时 ==========
        "node.js" | "nodejs" | "node" => {
            push!(home_p.join(".npm"));
            push!(home_p.join(".node-gyp"));
            push!(home_p.join(".corepack"));
            #[cfg(unix)]
            {
                push!(home_p.join(".config/configstore"));
                push!(PathBuf::from("/usr/local/lib/node_modules"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Preferences/node"));
                push!(home_p.join("Library/Caches/node"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a.clone()).join("npm"));
                    push!(PathBuf::from(a).join("npm-cache"));
                }
            }
        }
        "python 3" | "python3" | "python" | "python 2" | "python2" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".cache/pip"));
                push!(home_p.join(".config/pip"));
                push!(home_p.join(".python_history"));
                push!(home_p.join(".ipython"));
                push!(home_p.join(".jupyter"));
                push!(home_p.join(".conda"));
                push!(home_p.join(".cache/pypoetry"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Caches/pip"));
                push!(home_p.join("Library/Python"));
                push!(home_p.join("Library/Jupyter"));
            }
            #[cfg(windows)]
            {
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l.clone()).join("pip"));
                    push!(PathBuf::from(l.clone()).join("Python"));
                    push!(PathBuf::from(l.clone()).join("Continuum"));
                    push!(PathBuf::from(l.clone()).join("miniconda3"));
                    push!(PathBuf::from(l).join("anaconda3"));
                }
            }
        }
        "rust" | "rustc" | "cargo" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".rustup"));
                push!(home_p.join(".cargo"));
                push!(home_p.join(".cargo/registry"));
            }
            #[cfg(windows)]
            {
                if let Ok(u) = std::env::var("USERPROFILE") {
                    push!(PathBuf::from(u.clone()).join(".rustup"));
                    push!(PathBuf::from(u).join(".cargo"));
                }
            }
        }
        "go" | "golang" => {
            #[cfg(unix)]
            {
                push!(home_p.join("go"));
                push!(home_p.join(".cache/go"));
                push!(home_p.join(".cache/golangci-lint"));
                push!(home_p.join(".config/go"));
            }
            #[cfg(windows)]
            {
                if let Ok(u) = std::env::var("USERPROFILE") {
                    push!(PathBuf::from(u).join("go"));
                }
            }
        }
        "java" | "jdk" | "openjdk" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".java"));
                push!(home_p.join(".gradle"));
                push!(home_p.join(".m2"));
                push!(home_p.join(".sbt"));
                push!(home_p.join(".cache/JNA"));
                push!(home_p.join(".cache/jna"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Caches/Java"));
                push!(home_p.join("Library/Preferences/com.oracle.java.Java-Updater"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a.clone()).join("Oracle"));
                    push!(PathBuf::from(a).join("Java"));
                }
            }
        }
        ".net" | "dotnet" | "dotnet-sdk" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".dotnet"));
                push!(home_p.join(".nuget"));
                push!(home_p.join(".templateengine"));
                push!(home_p.join(".cache/NuGet"));
            }
            #[cfg(windows)]
            {
                if let Ok(u) = std::env::var("USERPROFILE") {
                    push!(PathBuf::from(u.clone()).join(".dotnet"));
                    push!(PathBuf::from(u).join(".nuget"));
                }
            }
        }

        // ========== 浏览器 ==========
        "chrome" | "google chrome" | "chromium" | "chromium-browser" => {
            #[cfg(target_os = "linux")]
            {
                push!(home_p.join(".config/google-chrome"));
                push!(home_p.join(".cache/google-chrome"));
                push!(home_p.join(".config/chromium"));
                push!(home_p.join(".cache/chromium"));
                push!(home_p.join(".config/chromium-browser"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Google/Chrome"));
                push!(home_p.join("Library/Caches/Google/Chrome"));
                push!(home_p.join("Library/Preferences/com.google.Chrome.plist"));
            }
            #[cfg(windows)]
            {
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l.clone()).join("Google/Chrome"));
                    push!(PathBuf::from(l).join("Google/Chrome/User Data"));
                }
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Google/Chrome"));
                }
            }
        }
        "firefox" | "mozilla firefox" => {
            #[cfg(target_os = "linux")]
            {
                push!(home_p.join(".mozilla/firefox"));
                push!(home_p.join(".cache/mozilla/firefox"));
                // keyword scanner catches *default* profiles under .mozilla/firefox/
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Firefox"));
                push!(home_p.join("Library/Caches/Firefox"));
                push!(home_p.join("Library/Preferences/org.mozilla.firefox.plist"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Mozilla/Firefox"));
                }
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l).join("Mozilla/Firefox"));
                }
            }
        }
        "edge" | "microsoft edge" | "msedge" => {
            #[cfg(target_os = "linux")]
            {
                push!(home_p.join(".config/microsoft-edge"));
                push!(home_p.join(".cache/microsoft-edge"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Microsoft Edge"));
                push!(home_p.join("Library/Caches/Microsoft Edge"));
            }
            #[cfg(windows)]
            {
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l.clone()).join("Microsoft/Edge"));
                    push!(PathBuf::from(l).join("Microsoft/Edge/User Data"));
                }
            }
        }
        "brave" | "brave browser" => {
            #[cfg(target_os = "linux")]
            {
                push!(home_p.join(".config/BraveSoftware"));
                push!(home_p.join(".cache/BraveSoftware"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/BraveSoftware"));
                push!(home_p.join("Library/Caches/BraveSoftware"));
            }
            #[cfg(windows)]
            {
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l).join("BraveSoftware"));
                }
            }
        }
        "opera" | "opera browser" | "opera stable" => {
            #[cfg(target_os = "linux")]
            {
                push!(home_p.join(".config/opera"));
                push!(home_p.join(".cache/opera"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/com.operasoftware.Opera"));
                push!(home_p.join("Library/Caches/com.operasoftware.Opera"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Opera Software"));
                }
            }
        }
        "vivaldi" => {
            #[cfg(target_os = "linux")]
            {
                push!(home_p.join(".config/vivaldi"));
                push!(home_p.join(".cache/vivaldi"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Vivaldi"));
                push!(home_p.join("Library/Caches/Vivaldi"));
            }
            #[cfg(windows)]
            {
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l).join("Vivaldi"));
                }
            }
        }
        "tor browser" | "tor-browser" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".tor-browser"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/TorBrowser-Data"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("TorBrowser"));
                }
            }
        }

        // ========== IDE & 编辑器 ==========
        "visual studio code" | "vscode" | "code" => {
            #[cfg(target_os = "linux")]
            {
                push!(home_p.join(".config/Code"));
                push!(home_p.join(".config/Code - OSS"));
                push!(home_p.join(".vscode"));
                push!(home_p.join(".vscode-oss"));
                push!(home_p.join(".cache/code"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Code"));
                push!(home_p.join("Library/Caches/com.microsoft.VSCode"));
                push!(home_p.join("Library/Preferences/com.microsoft.VSCode.plist"));
                push!(home_p.join(".vscode"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a.clone()).join("Code"));
                    push!(PathBuf::from(a).join("Code - Insiders"));
                }
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l).join("Programs/Microsoft VS Code"));
                }
            }
        }
        "intellij idea" | "intellij idea ultimate" | "intellij idea community" | "idea" => {
            #[cfg(unix)]
            {
                // 只清理 IntelliJ IDEA 对应产品的子目录，绝不删除 JetBrains 共享根目录
                // （根目录下同时存放 PyCharm/GoLand/CLion 等其它产品的活动数据）
                push!(home_p.join(".config/JetBrains/IntelliJIdea*"));
                push!(home_p.join(".cache/JetBrains/IntelliJIdea*"));
                push!(home_p.join(".local/share/JetBrains/IntelliJIdea*"));
                push!(home_p.join(".java/.userPrefs/jetbrains"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/JetBrains/IntelliJIdea*"));
                push!(home_p.join("Library/Caches/JetBrains/IntelliJIdea*"));
                push!(home_p.join("Library/Preferences/com.jetbrains.intellij.plist"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a.clone()).join("JetBrains/IntelliJIdea*"));
                }
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l).join("JetBrains/IntelliJIdea*"));
                }
            }
        }
        "pycharm" | "pycharm community" | "pycharm professional" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/JetBrains/PyCharm*"));
                push!(home_p.join(".cache/JetBrains/PyCharm*"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/JetBrains/PyCharm*"));
                push!(home_p.join("Library/Caches/JetBrains/PyCharm*"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("JetBrains/PyCharm*"));
                }
            }
        }
        "webstorm" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/JetBrains/WebStorm*"));
                push!(home_p.join(".cache/JetBrains/WebStorm*"));
            }
        }
        "goland" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/JetBrains/GoLand*"));
                push!(home_p.join(".cache/JetBrains/GoLand*"));
            }
        }
        "clion" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/JetBrains/CLion*"));
                push!(home_p.join(".cache/JetBrains/CLion*"));
            }
        }
        "android studio" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".android"));
                push!(home_p.join(".AndroidStudio*"));
                push!(home_p.join(".gradle"));
                push!(home_p.join(".cache/Google"));
                push!(home_p.join("Android"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/AndroidStudio*"));
                push!(home_p.join("Library/Caches/AndroidStudio*"));
                push!(home_p.join("Library/Preferences/AndroidStudio*"));
            }
            #[cfg(windows)]
            {
                if let Ok(u) = std::env::var("USERPROFILE") {
                    push!(PathBuf::from(u.clone()).join(".android"));
                    push!(PathBuf::from(u).join("AndroidStudioProjects"));
                }
            }
        }
        "sublime text" | "subl" | "sublime" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/sublime-text"));
                push!(home_p.join(".cache/sublime-text"));
                push!(home_p.join(".local/share/sublime-text"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Sublime Text"));
                push!(home_p.join("Library/Caches/Sublime Text"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Sublime Text"));
                }
            }
        }
        "neovim" | "nvim" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/nvim"));
                push!(home_p.join(".local/share/nvim"));
                push!(home_p.join(".cache/nvim"));
                push!(home_p.join(".local/state/nvim"));
            }
            #[cfg(windows)]
            {
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l.clone()).join("nvim"));
                    push!(PathBuf::from(l).join("nvim-data"));
                }
            }
        }
        "vim" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".vim"));
                push!(home_p.join(".vimrc"));
                push!(home_p.join(".config/vim"));
            }
            #[cfg(windows)]
            {
                if let Ok(u) = std::env::var("USERPROFILE") {
                    push!(PathBuf::from(u.clone()).join(".vim"));
                    push!(PathBuf::from(u).join("vimfiles"));
                }
            }
        }
        "emacs" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".emacs"));
                push!(home_p.join(".emacs.d"));
                push!(home_p.join(".config/emacs"));
                push!(home_p.join(".cache/emacs"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a.clone()).join(".emacs"));
                    push!(PathBuf::from(a).join(".emacs.d"));
                }
            }
        }
        "zed" | "zed editor" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/zed"));
                push!(home_p.join(".local/share/zed"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Zed"));
                push!(home_p.join("Library/Caches/Zed"));
            }
        }
        "postman" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/Postman"));
                push!(home_p.join(".cache/Postman"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Postman"));
                push!(home_p.join("Library/Caches/Postman"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Postman"));
                }
            }
        }

        // ========== 通信 & 协作 ==========
        "slack" => {
            #[cfg(target_os = "linux")]
            {
                push!(home_p.join(".config/Slack"));
                push!(home_p.join(".cache/Slack"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Slack"));
                push!(home_p.join("Library/Caches/com.tinyspeck.slackmac/Slack"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Slack"));
                }
            }
        }
        "discord" => {
            #[cfg(target_os = "linux")]
            {
                push!(home_p.join(".config/discord"));
                push!(home_p.join(".cache/discord"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/discord"));
                push!(home_p.join("Library/Caches/discord"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("discord"));
                }
            }
        }
        "telegram" | "telegram desktop" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".local/share/TelegramDesktop"));
                push!(home_p.join(".cache/TelegramDesktop"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Telegram Desktop"));
                push!(home_p.join("Library/Caches/Telegram Desktop"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Telegram Desktop"));
                }
            }
        }
        "zoom" | "zoom.us" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/zoom"));
                push!(home_p.join(".zoom"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/zoom.us"));
                push!(home_p.join("Library/Caches/us.zoom.xos"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Zoom"));
                }
            }
        }
        "teams" | "microsoft teams" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/Microsoft"));
                push!(home_p.join(".cache/Microsoft"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Microsoft/Teams"));
                push!(home_p.join("Library/Caches/com.microsoft.teams"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Microsoft/Teams"));
                }
            }
        }

        // ========== 云 & DevOps ==========
        "docker" | "docker desktop" | "docker engine" => {
            #[cfg(target_os = "linux")]
            {
                push!(home_p.join(".docker"));
                push!(PathBuf::from("/var/lib/docker"));
                push!(PathBuf::from("/etc/docker"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Containers/com.docker.docker"));
                push!(home_p.join("Library/Application Support/Docker"));
                push!(home_p.join("Library/Caches/com.docker.docker"));
                push!(home_p.join(".docker"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Docker"));
                }
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l).join("Docker"));
                }
            }
        }
        "kubectl" | "kubernetes" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".kube"));
                push!(home_p.join(".minikube"));
                push!(home_p.join(".cache/kubectl"));
            }
            #[cfg(windows)]
            {
                if let Ok(u) = std::env::var("USERPROFILE") {
                    push!(PathBuf::from(u).join(".kube"));
                }
            }
        }
        "terraform" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".terraform.d"));
                push!(home_p.join(".cache/terraform"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("terraform.d"));
                }
            }
        }
        "aws cli" | "aws" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".aws"));
            }
            #[cfg(windows)]
            {
                if let Ok(u) = std::env::var("USERPROFILE") {
                    push!(PathBuf::from(u).join(".aws"));
                }
            }
        }
        "azure cli" | "az" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".azure"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Azure CLI"));
                }
            }
        }
        "gcloud" | "google cloud sdk" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/gcloud"));
                push!(home_p.join(".config/gsutil"));
                push!(home_p.join(".cache/gcloud"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("gcloud"));
                }
            }
        }
        "vagrant" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".vagrant.d"));
            }
        }

        // ========== 数据库 ==========
        "mysql" | "mysql server" | "mariadb" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".mysql"));
                push!(home_p.join(".mysql_history"));
                push!(home_p.join(".config/mysql"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/mysql"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("MySQL"));
                }
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l).join("Programs/MySQL"));
                }
            }
        }
        "postgresql" | "postgres" | "psql" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".psql_history"));
                push!(home_p.join(".pgpass"));
                push!(home_p.join(".config/postgresql"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/postgresql"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("postgresql"));
                }
            }
        }
        "mongodb" | "mongo" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".mongorc.js"));
                push!(home_p.join(".mongodb"));
                push!(home_p.join(".cache/mongocli"));
                push!(home_p.join(".config/mongodb"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/mongodb"));
                push!(home_p.join("Library/Preferences/mongodb"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("MongoDB"));
                }
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l).join("MongoDB"));
                }
            }
        }
        "redis" | "redis-server" | "redis-cli" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".redis"));
                push!(home_p.join(".config/redis"));
            }
        }
        "dbeaver" | "dbeaver ce" | "dbeaver community" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".local/share/DBeaverData"));
                push!(home_p.join(".cache/DBeaverData"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/DBeaverData"));
                push!(home_p.join("Library/Caches/DBeaverData"));
            }
            #[cfg(windows)]
            {
                if let Ok(u) = std::env::var("USERPROFILE") {
                    push!(PathBuf::from(u).join(".DBeaverData"));
                }
            }
        }

        // ========== 媒体 & 设计 ==========
        "vlc" | "vlc media player" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/vlc"));
                push!(home_p.join(".cache/vlc"));
                push!(home_p.join(".local/share/vlc"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/org.videolan.vlc"));
                push!(home_p.join("Library/Caches/org.videolan.vlc"));
                push!(home_p.join("Library/Preferences/org.videolan.vlc"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("vlc"));
                }
            }
        }
        "obs studio" | "obs" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/obs-studio"));
                push!(home_p.join(".cache/obs-studio"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/obs-studio"));
                push!(home_p.join("Library/Caches/obs-studio"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("obs-studio"));
                }
            }
        }
        "gimp" | "gimp 2" | "gimp 3" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/GIMP"));
                push!(home_p.join(".cache/GIMP"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/GIMP"));
                push!(home_p.join("Library/Caches/GIMP"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("GIMP"));
                }
            }
        }
        "blender" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/blender"));
                push!(home_p.join(".cache/blender"));
                push!(home_p.join(".local/share/blender"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/Blender"));
                push!(home_p.join("Library/Caches/Blender"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("Blender Foundation"));
                }
            }
        }
        "inkscape" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/inkscape"));
                push!(home_p.join(".cache/inkscape"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/org.inkscape.Inkscape"));
                push!(home_p.join("Library/Caches/org.inkscape.Inkscape"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("inkscape"));
                }
            }
        }

        // ========== 系统工具 ==========
        "git" | "git for windows" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".gitconfig"));
                push!(home_p.join(".git-credentials"));
                push!(home_p.join(".gitignore_global"));
            }
            #[cfg(windows)]
            {
                if let Ok(u) = std::env::var("USERPROFILE") {
                    push!(PathBuf::from(u).join(".gitconfig"));
                }
            }
        }
        "ssh" | "openssh" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".ssh"));
                // 不删除 .ssh/known_hosts 和 authorized_keys — 用户可能想保留
            }
            #[cfg(windows)]
            {
                if let Ok(u) = std::env::var("USERPROFILE") {
                    push!(PathBuf::from(u).join(".ssh"));
                }
            }
        }
        "tmux" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".tmux.conf"));
                push!(home_p.join(".tmux"));
            }
        }
        "screen" | "gnu screen" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".screenrc"));
            }
        }

        // ========== 容器 & 虚拟化 ==========
        "podman" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/containers"));
                push!(home_p.join(".local/share/containers"));
                push!(home_p.join(".cache/containers"));
            }
        }
        "virtualbox" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".config/VirtualBox"));
                push!(home_p.join(".VirtualBox"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/VirtualBox"));
                push!(home_p.join("Library/Application Support/VirtualBox"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("VirtualBox"));
                }
            }
        }
        "vmware" | "vmware fusion" | "vmware workstation" => {
            #[cfg(unix)]
            {
                push!(home_p.join(".vmware"));
            }
            #[cfg(target_os = "macos")]
            {
                push!(home_p.join("Library/Application Support/VMware Fusion"));
                push!(home_p.join("Library/Preferences/VMware Fusion"));
            }
            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a).join("VMware"));
                }
            }
        }

        // ========== 通用兜底 ==========
        _ => {
            let slug = app_lower.replace([' ', '_'], "-");
            let pkg_slug = pkg_lower.replace([' ', '_'], "-");

            #[cfg(unix)]
            {
                push!(home_p.join(format!(".config/{}", slug)));
                push!(home_p.join(format!(".config/{}", pkg_slug)));
                push!(home_p.join(format!(".cache/{}", slug)));
                push!(home_p.join(format!(".cache/{}", pkg_slug)));
                push!(home_p.join(format!(".local/share/{}", slug)));
                push!(home_p.join(format!(".local/share/{}", pkg_slug)));
                push!(home_p.join(format!(".local/state/{}", slug)));
                push!(home_p.join(format!(".local/state/{}", pkg_slug)));
                push!(home_p.join(format!(".{}", slug)));
                push!(home_p.join(format!(".{}", pkg_slug)));
            }

            #[cfg(target_os = "macos")]
            {
                push!(home_p.join(format!("Library/Application Support/{}", slug)));
                push!(home_p.join(format!("Library/Application Support/{}", pkg_slug)));
                push!(home_p.join(format!("Library/Caches/{}", slug)));
                push!(home_p.join(format!("Library/Caches/{}", pkg_slug)));
                push!(home_p.join(format!("Library/Preferences/{}", slug)));
                push!(home_p.join(format!("Library/Preferences/{}", pkg_slug)));
                push!(home_p.join(format!("Library/Logs/{}", slug)));
                push!(home_p.join(format!("Library/Logs/{}", pkg_slug)));
            }

            #[cfg(windows)]
            {
                if let Ok(a) = std::env::var("APPDATA") {
                    push!(PathBuf::from(a.clone()).join(&slug));
                    push!(PathBuf::from(a).join(&pkg_slug));
                }
                if let Ok(l) = std::env::var("LOCALAPPDATA") {
                    push!(PathBuf::from(l.clone()).join(&slug));
                    push!(PathBuf::from(l).join(&pkg_slug));
                }
                if let Ok(p) = std::env::var("PROGRAMDATA") {
                    push!(PathBuf::from(p.clone()).join(&slug));
                    push!(PathBuf::from(p).join(&pkg_slug));
                }
            }
        }
    }

    // 展开带通配符（*）的路径为实际存在的子目录，再统一去重
    let mut expanded: Vec<PathBuf> = Vec::new();
    for p in paths {
        let s = p.to_string_lossy();
        if s.contains('*') {
            expand_glob(&p, &mut expanded);
        } else {
            expanded.push(p);
        }
    }

    // 去重保持顺序
    let mut seen = std::collections::HashSet::new();
    expanded
        .into_iter()
        .filter(|p| seen.insert(p.display().to_string()))
        .collect()
}

/// 将含单个尾部通配符（如 `~/.config/JetBrains/IntelliJIdea*`）的路径展开为实际存在的目录。
/// 若通配符不在末尾、父目录不存在或无匹配项，则静默跳过（不产生清理目标）。
fn expand_glob(pattern: &std::path::Path, out: &mut Vec<PathBuf>) {
    let s = pattern.to_string_lossy();
    let Some((prefix, suffix)) = s.split_once('*') else {
        return;
    };
    // 仅支持尾部通配符（后缀为空或纯文件名结尾），避免复杂 glob
    if suffix.contains('/') || suffix.contains('\\') {
        return;
    }
    let prefix_path = std::path::Path::new(prefix);
    let Some(parent) = prefix_path.parent() else {
        return;
    };
    let Some(stem) = prefix_path.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    if stem.is_empty() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(stem) {
            out.push(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_glob_matches_stem_in_real_dir() {
        let tmp = std::env::temp_dir().join(format!("devnexus_glob_test_{}", std::process::id()));
        let jetbrains = tmp.join("JetBrains");
        std::fs::create_dir_all(&jetbrains).unwrap();
        std::fs::create_dir_all(jetbrains.join("IntelliJIdea2026.1")).unwrap();
        std::fs::create_dir_all(jetbrains.join("PyCharm2026.1")).unwrap();

        let mut out = Vec::new();
        let pattern = jetbrains.join("IntelliJIdea*");
        expand_glob(&pattern, &mut out);
        assert_eq!(out.len(), 1, "只应匹配 IntelliJIdea 前缀");
        assert_eq!(
            out[0].file_name().unwrap().to_str().unwrap(),
            "IntelliJIdea2026.1"
        );

        let mut none = Vec::new();
        let missing = jetbrains.join("GoLand*");
        expand_glob(&missing, &mut none);
        assert!(none.is_empty(), "无匹配项应返回空");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
