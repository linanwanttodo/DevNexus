use std::fs;
use std::path::{Path, PathBuf};

/// 按 $SHELL 选择用户的 rc 文件：zsh -> .zshrc，bash -> .bashrc，其他 -> .profile
pub fn detect_shell_rc(home: &Path) -> PathBuf {
    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.ends_with("zsh") {
        home.join(".zshrc")
    } else if shell.ends_with("bash") {
        home.join(".bashrc")
    } else {
        home.join(".profile")
    }
}

/// 写入/替换 `export KEY="value"` 行到 rc 文件。
///
/// 先调用 `crate::utils::validate_rc_value` 校验 value；若 rc 中尚未包含 key 则追加，
/// 否则将包含 key 的行替换为新的 export 行。返回 `Ok(true)` 表示新增，`Ok(false)` 表示已替换。
pub fn set_export_line(home: &Path, key: &str, value: &str) -> Result<bool, String> {
    crate::utils::validate_rc_value(value)?;
    let rc_path = detect_shell_rc(home);
    let export_line = format!("\nexport {}=\"{}\"\n", key, value);
    let existing = fs::read_to_string(&rc_path).unwrap_or_default();
    if !existing.contains(key) {
        fs::write(&rc_path, format!("{}{}", existing, export_line))
            .map_err(|e| format!("Failed to write {}: {}", rc_path.display(), e))?;
        Ok(true)
    } else {
        let updated = existing
            .lines()
            .map(|line| {
                if line.contains(key) {
                    export_line.trim().to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&rc_path, updated)
            .map_err(|e| format!("Failed to write {}: {}", rc_path.display(), e))?;
        Ok(false)
    }
}

/// 写入 `# DevNexus: {env_name}` 注释 + `export PATH="{path}:$PATH"` 到 rc 文件。
///
/// 与 environment.rs 原有格式一致（追加式写入，去重由调用方负责）。
/// 返回 `Ok(true)` 表示写入发生。
pub fn set_path_line(home: &Path, env_name: &str, path: &str) -> Result<bool, String> {
    crate::utils::validate_rc_value(path)?;
    let rc_path = detect_shell_rc(home);
    let export_line = format!(
        "\n# DevNexus: {}\nexport PATH=\"{}:$PATH\"\n",
        env_name, path
    );
    let existing = fs::read_to_string(&rc_path).unwrap_or_default();
    fs::write(&rc_path, format!("{}{}", existing, export_line))
        .map_err(|e| format!("Failed to write {}: {}", rc_path.display(), e))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // detect_shell_rc 依赖 $SHELL 环境变量，测试间用互斥锁串行化避免并发竞争
    static SHELL_LOCK: Mutex<()> = Mutex::new(());

    fn temp_home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rc_editor_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp home");
        dir
    }

    #[test]
    fn test_detect_shell_rc() {
        let _guard = SHELL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = temp_home("detect_shell_rc");

        std::env::set_var("SHELL", "/bin/zsh");
        assert_eq!(detect_shell_rc(&home), home.join(".zshrc"));

        std::env::set_var("SHELL", "/bin/bash");
        assert_eq!(detect_shell_rc(&home), home.join(".bashrc"));

        std::env::set_var("SHELL", "/usr/bin/fish");
        assert_eq!(detect_shell_rc(&home), home.join(".profile"));

        std::env::remove_var("SHELL");
        assert_eq!(detect_shell_rc(&home), home.join(".profile"));
    }

    #[test]
    fn test_set_export_line_adds_new() {
        let _guard = SHELL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = temp_home("set_export_line_add");
        std::env::set_var("SHELL", "/bin/bash");

        let wrote = set_export_line(&home, "GOPROXY", "https://proxy.golang.com.cn,direct")
            .expect("set export line");
        assert!(wrote);

        let content = fs::read_to_string(home.join(".bashrc")).expect("read rc");
        assert!(content.contains("export GOPROXY=\"https://proxy.golang.com.cn,direct\""));
    }

    #[test]
    fn test_set_export_line_replaces_existing() {
        let _guard = SHELL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = temp_home("set_export_line_replace");
        std::env::set_var("SHELL", "/bin/bash");
        fs::write(
            home.join(".bashrc"),
            "export GOPROXY=\"https://old.example.com\"\n",
        )
        .expect("seed rc");

        let wrote = set_export_line(&home, "GOPROXY", "https://new.example.com")
            .expect("replace export line");
        assert!(!wrote);

        let content = fs::read_to_string(home.join(".bashrc")).expect("read rc");
        assert!(content.contains("export GOPROXY=\"https://new.example.com\""));
        assert!(!content.contains("old.example.com"));
    }

    #[test]
    fn test_set_export_line_invalid_value() {
        let _guard = SHELL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = temp_home("set_export_line_invalid");
        std::env::set_var("SHELL", "/bin/bash");

        let res = set_export_line(&home, "GOPROXY", "\"; rm -rf ~; #");
        assert!(res.is_err());
        assert!(!home.join(".bashrc").exists());
    }

    #[test]
    fn test_set_path_line_writes_comment_and_export() {
        let _guard = SHELL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = temp_home("set_path_line");
        std::env::set_var("SHELL", "/bin/bash");

        let wrote =
            set_path_line(&home, "Java", "/usr/lib/jvm/java-17/bin").expect("set path line");
        assert!(wrote);

        let content = fs::read_to_string(home.join(".bashrc")).expect("read rc");
        assert!(content.contains("# DevNexus: Java"));
        assert!(content.contains("export PATH=\"/usr/lib/jvm/java-17/bin:$PATH\""));
    }
}
