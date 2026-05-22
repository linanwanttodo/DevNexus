use crate::residue_scanner::{dir_size, ResidueItem};
use std::path::PathBuf;

/// 通过关键词在常见残留目录中递归扫描匹配的文件/目录
pub fn scan_by_keywords(app_name: &str, home: &str) -> (Vec<ResidueItem>, Vec<ResidueItem>) {
    let keywords = build_keywords(app_name);
    let home_path = std::path::Path::new(home);

    // 定义要扫描的根目录
    let mut roots: Vec<PathBuf> = Vec::new();

    // Linux / macOS
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        roots.push(home_path.join(".config"));
        roots.push(home_path.join(".cache"));
        roots.push(home_path.join(".local/share"));
        roots.push(home_path.join(".local/state"));
    }

    // macOS
    #[cfg(target_os = "macos")]
    {
        roots.push(home_path.join("Library/Application Support"));
        roots.push(home_path.join("Library/Caches"));
        roots.push(home_path.join("Library/Preferences"));
        roots.push(home_path.join("Library/Logs"));
        roots.push(home_path.join("Library/Containers"));
    }

    // Windows
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            roots.push(PathBuf::from(appdata));
        }
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            roots.push(PathBuf::from(localappdata));
        }
        if let Ok(progdata) = std::env::var("PROGRAMDATA") {
            roots.push(PathBuf::from(progdata));
        }
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            roots.push(PathBuf::from(userprofile).join("AppData"));
        }
    }

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for root in &roots {
        if !root.exists() || !root.is_dir() {
            continue;
        }
        let walker = walkdir::WalkDir::new(root)
            .max_depth(4)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_hidden_or_sys(e));

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();
            let fname = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();

            if matches_keywords(&fname, &keywords) {
                let p_str = path.display().to_string();
                if seen.insert(p_str.clone()) {
                    let size = if path.is_dir() {
                        dir_size(path)
                    } else {
                        path.metadata().map(|m| m.len()).unwrap_or(0)
                    };
                    let category = if fname.contains("cache") || fname.contains("缓存") {
                        "cache"
                    } else if fname.contains("config") || fname.contains("pref") {
                        "config"
                    } else if fname.contains("log") {
                        "log"
                    } else {
                        "data"
                    };
                    let item = ResidueItem {
                        path: p_str,
                        size,
                        category: category.into(),
                        is_safe_to_delete: true,
                        description: String::new(),
                    };
                    if path.is_dir() {
                        dirs.push(item);
                    } else if path.is_file() {
                        files.push(item);
                    }
                }
            }
        }
    }

    (dirs, files)
}

fn is_hidden_or_sys(entry: &walkdir::DirEntry) -> bool {
    let fname = entry.file_name().to_str().unwrap_or("");
    // Skip hidden dirs at root except .config/.cache/.local
    if entry.depth() == 1 && fname.starts_with('.') {
        let allowed = [".config", ".cache", ".local"];
        if !allowed.contains(&fname) {
            return true;
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(meta) = entry.metadata() {
            if meta.file_attributes() & 0x2 != 0 {
                // FILE_ATTRIBUTE_HIDDEN
                return true;
            }
        }
    }
    false
}

fn build_keywords(app_name: &str) -> Vec<String> {
    let lower = app_name.to_lowercase();
    let mut words: Vec<String> = Vec::new();

    // 原始名称
    words.push(lower.clone());

    // 按空格/连字符/下划线分割
    for sep in &[' ', '-', '_', '.'] {
        for part in lower.split(*sep) {
            let part = part.trim();
            if part.len() >= 3 && !words.contains(&part.to_string()) {
                words.push(part.to_string());
            }
        }
    }

    // 特殊常见缩写映射
    let aliases = match lower.as_str() {
        "visual studio code" | "vscode" | "code" => {
            vec!["vscode", "visual studio code", "code"]
        }
        "google chrome" | "chrome" => vec!["chrome", "google-chrome"],
        "mozilla firefox" | "firefox" => vec!["firefox", "mozilla", "firefox-esr"],
        "microsoft edge" | "edge" => vec!["edge", "msedge", "microsoft-edge"],
        "intellij idea" | "idea" => vec!["intellij", "idea", "jetbrains"],
        "postman" => vec!["postman", "postman-agent"],
        "docker" | "docker desktop" => vec!["docker"],
        "node.js" | "nodejs" | "node" => vec!["node", "nodejs", "npm"],
        p if p.contains("python") => vec!["python", "pip", "conda"],
        p if p.contains("rust") => vec!["rust", "cargo", "rustup"],
        p if p.contains("golang") || p.contains("go ") => vec!["go", "golang"],
        _ => vec![],
    };
    for a in aliases {
        if !words.contains(&a.to_string()) {
            words.push(a.to_string());
        }
    }

    words
}

fn matches_keywords(fname: &str, keywords: &[String]) -> bool {
    // 精确匹配
    for kw in keywords {
        if fname == kw {
            return true;
        }
        // 包含匹配
        if fname.contains(kw) {
            return true;
        }
    }
    false
}
