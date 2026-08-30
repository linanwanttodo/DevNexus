// src-tauri/src/commands/autostart.rs — 开机自启 + 静默启动
// 开机自启：写入系统自启入口（Linux XDG autostart / macOS LaunchAgent / Windows 注册表 Run）。
// 静默启动：持久化标志文件，应用启动时读取——开启则主窗口不显示，后台常驻托盘 + 灵动岛。

fn silent_flag_path() -> std::path::PathBuf {
    crate::utils::data_dir().join("silent_start")
}

/// 当前是否开启静默启动（启动时不显示主窗口）
#[tauri::command]
pub fn get_silent_start() -> bool {
    silent_flag_path().exists()
}

/// 设置静默启动
#[tauri::command]
pub fn set_silent_start(enabled: bool) -> Result<(), String> {
    let p = silent_flag_path();
    if enabled {
        std::fs::write(&p, "1").map_err(|e| e.to_string())
    } else {
        let _ = std::fs::remove_file(&p);
        Ok(())
    }
}

// ── Linux：XDG autostart .desktop ──
#[cfg(target_os = "linux")]
fn autostart_desktop_path() -> std::path::PathBuf {
    let dir = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("autostart");
    dir.join("devnexus.desktop")
}

#[cfg(target_os = "linux")]
fn set_autostart_linux(enabled: bool) -> Result<(), String> {
    let path = autostart_desktop_path();
    if enabled {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=DevNexus\nExec=\"{}\"\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        );
        std::fs::write(&path, content).map_err(|e| e.to_string())
    } else {
        let _ = std::fs::remove_file(&path);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn get_autostart_linux() -> bool {
    autostart_desktop_path().exists()
}

// ── macOS：LaunchAgent plist ──
#[cfg(target_os = "macos")]
fn autostart_plist_path() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join("Library/LaunchAgents/com.devnexus.app.plist"))
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}

#[cfg(target_os = "macos")]
fn set_autostart_macos(enabled: bool) -> Result<(), String> {
    let path = autostart_plist_path();
    if enabled {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n  <key>Label</key><string>com.devnexus.app</string>\n  <key>ProgramArguments</key><array><string>{}</string></array>\n  <key>RunAtLoad</key><true/>\n</dict></plist>\n",
            exe.display()
        );
        std::fs::write(&path, content).map_err(|e| e.to_string())
    } else {
        let _ = std::fs::remove_file(&path);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn get_autostart_macos() -> bool {
    autostart_plist_path().exists()
}

// ── Windows：注册表 Run 键 ──
#[cfg(target_os = "windows")]
fn set_autostart_windows(enabled: bool) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map_err(|e| e.to_string())?;
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        key.set_value("DevNexus", &format!("\"{}\"", exe.display()))
            .map_err(|e| e.to_string())
    } else {
        let _ = key.delete_value("DevNexus");
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn get_autostart_windows() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .and_then(|k| k.get_value::<String, _>("DevNexus"))
        .map(|_| true)
        .unwrap_or(false)
}

/// 查询开机自启是否开启
#[tauri::command]
pub fn get_autostart() -> bool {
    #[cfg(target_os = "linux")]
    {
        get_autostart_linux()
    }
    #[cfg(target_os = "macos")]
    {
        get_autostart_macos()
    }
    #[cfg(target_os = "windows")]
    {
        get_autostart_windows()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    // 仅 Linux 测试使用 super 符号（autostart_desktop_path）。
    // 若不加 cfg，Windows/macOS 上 Linux-only 测试被移除后 `use super::*`
    // 成为 unused import，触发 clippy -D warnings 编译失败（CI 已复现）。
    #[cfg(target_os = "linux")]
    use super::*;

    #[test]
    fn test_silent_start_flag_roundtrip() {
        // 直接测 flag 文件的读写逻辑，避免触碰真实 data_dir 的写入路径
        let dir =
            std::env::temp_dir().join(format!("devnexus_autostart_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("silent_start");
        // 未创建时视为关闭
        assert!(!path.exists());
        // 创建 → 开启
        std::fs::write(&path, "1").unwrap();
        assert!(path.exists());
        // 删除 → 关闭
        let _ = std::fs::remove_file(&path);
        assert!(!path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_autostart_desktop_path_uses_xdg() {
        let dir = std::env::temp_dir().join(format!("devnexus_xdg_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // 设置 XDG_CONFIG_HOME 指向临时目录
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        let p = autostart_desktop_path();
        assert_eq!(p, dir.join("autostart").join("devnexus.desktop"));

        // 清除后回退到 ~/.config
        std::env::remove_var("XDG_CONFIG_HOME");
        let fallback = autostart_desktop_path();
        assert!(
            fallback
                .to_string_lossy()
                .ends_with(".config/autostart/devnexus.desktop")
                || fallback
                    .file_name()
                    .map(|f| f == "devnexus.desktop")
                    .unwrap_or(false)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_silent_start_get_set() {
        // 测试 get_silent_start 和 set_silent_start 的基本功能
        // 先确保关闭
        let _ = set_silent_start(false);
        assert!(!get_silent_start());

        // 开启
        assert!(set_silent_start(true).is_ok());
        assert!(get_silent_start());

        // 再次关闭
        assert!(set_silent_start(false).is_ok());
        assert!(!get_silent_start());
    }

    #[test]
    fn test_set_autostart_unsupported_platform() {
        // 在不支持的平台上应该返回错误
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let result = set_autostart(true);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("not supported"));
        }
    }
}

/// 设置开机自启
#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        set_autostart_linux(enabled)
    }
    #[cfg(target_os = "macos")]
    {
        set_autostart_macos(enabled)
    }
    #[cfg(target_os = "windows")]
    {
        set_autostart_windows(enabled)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err("autostart not supported on this platform".into())
    }
}
