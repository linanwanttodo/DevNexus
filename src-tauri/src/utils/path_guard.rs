// src-tauri/src/utils/path_guard.rs
//! 前端传入路径的基础防线：绝对路径、长度、无遍历分量、无控制字符。
//!
//! 注意：不在此处做敏感目录黑名单——SSH 私钥导入等合法功能需要读取 `~/.ssh`。
//! 各命令应在此基础上叠加自己的业务约束（大小上限、扩展名、JSON schema 等）。

use std::path::PathBuf;

/// 单个路径参数的最大长度
pub const MAX_PATH_LEN: usize = 4096;

/// Windows 风格绝对路径（`X:\...`、`X:/...`、UNC `\\...`）。
/// 在非 Windows 平台上 `Path::is_absolute()` 不识别盘符，这里单独判断，
/// 保证校验逻辑在任意构建平台上行为一致。
fn is_windows_absolute(p: &str) -> bool {
    if p.starts_with("\\\\") {
        return true;
    }
    let b = p.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// 校验前端传入的本地文件路径，返回规范化前的 PathBuf。
///
/// 拒绝：空/超长、相对路径、含 `..` 或单独 `.` 分量、含 NUL/换行等控制字符。
pub fn validate_abs_sane_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Path is empty".to_string());
    }
    if trimmed.len() > MAX_PATH_LEN {
        return Err("Path is too long".to_string());
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err("Path contains control characters".to_string());
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() && !is_windows_absolute(trimmed) {
        return Err("Path must be absolute".to_string());
    }
    for comp in path.components() {
        use std::path::Component;
        match comp {
            Component::ParentDir => {
                return Err("Path traversal ('..') is not allowed".to_string());
            }
            Component::CurDir => {
                return Err("Redundant '.' component is not allowed".to_string());
            }
            _ => {}
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_absolute_paths() {
        assert!(validate_abs_sane_path("/home/user/downloads/a.json").is_ok());
        assert!(validate_abs_sane_path("/tmp/devnexus-install/node.tar.gz").is_ok());
        assert!(validate_abs_sane_path("C:\\Users\\u\\Downloads\\a.json").is_ok());
        assert!(validate_abs_sane_path("/home/u/.ssh/id_ed25519").is_ok()); // 合法功能需要
    }

    #[test]
    fn rejects_empty_and_overlong() {
        assert!(validate_abs_sane_path("").is_err());
        assert!(validate_abs_sane_path("   ").is_err());
        let long = "/".to_string() + &"a".repeat(5000);
        assert!(validate_abs_sane_path(&long).is_err());
    }

    #[test]
    fn rejects_relative_paths() {
        assert!(validate_abs_sane_path("relative/path.txt").is_err());
        assert!(validate_abs_sane_path("./local.txt").is_err());
    }

    #[test]
    fn rejects_traversal_components() {
        assert!(validate_abs_sane_path("/home/user/../../etc/shadow").is_err());
        assert!(validate_abs_sane_path("/safe/../secret").is_err());
    }

    #[test]
    fn rejects_control_characters() {
        assert!(validate_abs_sane_path("/tmp/a\nb").is_err());
        assert!(validate_abs_sane_path("/tmp/a\0b").is_err());
    }
}
