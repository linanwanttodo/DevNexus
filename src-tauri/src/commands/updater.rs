use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct GithubRelease {
    pub tag_name: String,
    pub html_url: String,
    pub body: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateInfo {
    pub has_update: bool,
    pub latest_version: String,
    pub current_version: String,
    pub download_url: String,
    pub release_notes: Option<String>,
    pub published_at: Option<String>,
}

/// Check GitHub Releases API for newer versions
#[tauri::command]
pub async fn check_for_updates_github() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    let client = reqwest::Client::builder()
        .user_agent("DevNexus/2.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get("https://api.github.com/repos/linanwanttodo/DevNexus/releases/latest")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch latest release: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("GitHub API 请求失败 (HTTP {}): {}", status, body));
    }

    let release: GithubRelease = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    // Strip leading 'v' from tag for comparison
    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let current = current_version.trim_start_matches('v').to_string();

    let has_update = match (
        semver::Version::parse(&latest_version),
        semver::Version::parse(&current),
    ) {
        (Ok(latest), Ok(current)) => latest > current,
        _ => false,
    };

    Ok(UpdateInfo {
        has_update,
        latest_version: release.tag_name,
        current_version,
        download_url: release.html_url,
        release_notes: release.body,
        published_at: release.published_at,
    })
}

/// Get the download URL for the current platform's installer
///
/// Detects architecture and installed bundle format at runtime to return
/// the correct download URL for the user's specific installation.
#[tauri::command]
pub fn get_download_url(version: String) -> Result<String, String> {
    let version = version.trim_start_matches('v');
    let tag = format!("v{}", version);
    let base = format!(
        "https://github.com/linanwanttodo/DevNexus/releases/download/{}",
        tag
    );

    #[cfg(target_os = "linux")]
    {
        // APPIMAGE env var is set when running from an AppImage
        if std::env::var("APPIMAGE").is_ok() {
            return Ok(format!("{}/DevNexus_{}_amd64.AppImage", base, version));
        }
        // System-installed (deb / rpm / manual)
        return Ok(format!("{}/DevNexus_{}_amd64.deb", base, version));
    }

    #[cfg(target_os = "macos")]
    {
        // Binary is arch-specific on macOS — compile-time detection is correct
        #[cfg(target_arch = "aarch64")]
        {
            return Ok(format!("{}/DevNexus_{}_aarch64.dmg", base, version));
        }
        #[cfg(target_arch = "x86_64")]
        {
            return Ok(format!("{}/DevNexus_{}_x64.dmg", base, version));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if is_nsis_install() {
            return Ok(format!("{}/DevNexus_{}_x64-setup.exe", base, version));
        }
        return Ok(format!("{}/DevNexus_{}_x64_zh-CN.msi", base, version));
    }

    #[allow(unreachable_code)]
    Err("Unsupported platform".to_string())
}

/// Detect whether the app was installed via NSIS on Windows by checking the
/// registry UninstallString for the NSIS uninstaller marker.
#[cfg(target_os = "windows")]
fn is_nsis_install() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall") {
        for name in key.enum_keys().filter_map(|k| k.ok()) {
            if let Ok(sub) = key.open_subkey(&name) {
                if let Ok(display_name) = sub.get_value::<String, _>("DisplayName") {
                    if display_name.contains("DevNexus") {
                        if let Ok(uninstall_str) = sub.get_value::<String, _>("UninstallString") {
                            // NSIS uses its own uninstaller exe; MSI uses msiexec
                            return uninstall_str.to_lowercase().contains("uninstall.exe");
                        }
                    }
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_comparison_newer() {
        assert!(
            semver::Version::parse("1.0.1").unwrap() > semver::Version::parse("1.0.0").unwrap()
        );
        assert!(
            semver::Version::parse("2.0.0").unwrap() > semver::Version::parse("1.9.9").unwrap()
        );
        assert!(
            semver::Version::parse("1.1.0").unwrap() > semver::Version::parse("1.0.9").unwrap()
        );
    }

    #[test]
    fn test_semver_comparison_older() {
        assert!(
            semver::Version::parse("1.0.0").unwrap() < semver::Version::parse("1.0.1").unwrap()
        );
        assert!(
            semver::Version::parse("0.9.9").unwrap() < semver::Version::parse("1.0.0").unwrap()
        );
    }

    #[test]
    fn test_semver_comparison_equal() {
        assert_eq!(
            semver::Version::parse("1.0.0").unwrap(),
            semver::Version::parse("1.0.0").unwrap()
        );
        assert_eq!(
            semver::Version::parse("2.5.10").unwrap(),
            semver::Version::parse("2.5.10").unwrap()
        );
    }

    #[test]
    fn test_semver_strip_v_prefix() {
        // semver itself rejects 'v' prefix, which is why check_for_updates_github strips it
        assert!(
            semver::Version::parse("1.0.0").unwrap() == semver::Version::parse("1.0.0").unwrap()
        );
        assert!("v1.0.0".trim_start_matches('v') == "1.0.0");
    }

    #[test]
    fn test_update_info_serialization() {
        let info = UpdateInfo {
            has_update: true,
            latest_version: "v2.0.0".to_string(),
            current_version: "1.0.0".to_string(),
            download_url: "https://github.com/linanwanttodo/DevNexus/releases/v2.0.0".to_string(),
            release_notes: Some("Bug fixes and improvements".to_string()),
            published_at: Some("2024-01-15T10:00:00Z".to_string()),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"has_update\":true"));
        assert!(json.contains("\"latest_version\":\"v2.0.0\""));
        assert!(json.contains("\"current_version\":\"1.0.0\""));
        assert!(json.contains("\"release_notes\":\"Bug fixes and improvements\""));

        // Deserialize back
        let deserialized: UpdateInfo = serde_json::from_str(&json).unwrap();
        assert!(deserialized.has_update);
        assert_eq!(deserialized.latest_version, "v2.0.0");
        assert_eq!(
            deserialized.release_notes.unwrap(),
            "Bug fixes and improvements"
        );
    }

    #[test]
    fn test_github_release_serialization() {
        let json = r#"{
            "tag_name": "v2.0.0",
            "html_url": "https://github.com/linanwanttodo/DevNexus/releases/tag/v2.0.0",
            "body": "New features added",
            "published_at": "2024-01-15T10:00:00Z"
        }"#;

        let release: GithubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v2.0.0");
        assert_eq!(release.body.unwrap(), "New features added");
        assert_eq!(release.published_at.unwrap(), "2024-01-15T10:00:00Z");
    }
}
