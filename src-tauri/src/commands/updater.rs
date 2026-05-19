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
        .user_agent("DevNexus-Updater/1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get("https://api.github.com/repos/linanwanttodo/DevNexus/releases/latest")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch latest release: {}", e))?;

    if !response.status().is_success() {
        return Ok(UpdateInfo {
            has_update: false,
            latest_version: "unknown".to_string(),
            current_version,
            download_url: String::new(),
            release_notes: None,
            published_at: None,
        });
    }

    let release: GithubRelease = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    // Strip leading 'v' from tag for comparison
    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let current = current_version.trim_start_matches('v').to_string();

    let has_update = compare_versions(&latest_version, &current) > 0;

    Ok(UpdateInfo {
        has_update,
        latest_version: release.tag_name,
        current_version,
        download_url: release.html_url,
        release_notes: release.body,
        published_at: release.published_at,
    })
}

/// Compare two semver strings. Returns >0 if a > b, <0 if a < b, 0 if equal.
fn compare_versions(a: &str, b: &str) -> i32 {
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();

    let max_len = a_parts.len().max(b_parts.len());
    for i in 0..max_len {
        let a_num = a_parts.get(i).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        let b_num = b_parts.get(i).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        if a_num != b_num {
            return a_num - b_num;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions_equal() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), 0);
        assert_eq!(compare_versions("2.5.10", "2.5.10"), 0);
        assert_eq!(compare_versions("0.0.0", "0.0.0"), 0);
    }

    #[test]
    fn test_compare_versions_newer() {
        // a > b => positive
        assert!(compare_versions("1.0.1", "1.0.0") > 0);
        assert!(compare_versions("1.1.0", "1.0.9") > 0);
        assert!(compare_versions("2.0.0", "1.9.9") > 0);
        assert!(compare_versions("1.0.0", "0.9.9") > 0);
    }

    #[test]
    fn test_compare_versions_older() {
        // a < b => negative
        assert!(compare_versions("1.0.0", "1.0.1") < 0);
        assert!(compare_versions("0.9.9", "1.0.0") < 0);
        assert!(compare_versions("1.0.9", "1.1.0") < 0);
    }

    #[test]
    fn test_compare_versions_different_length() {
        assert_eq!(compare_versions("1.0", "1.0.0"), 0);
        assert!(compare_versions("1.0.1", "1.0") > 0);
        assert!(compare_versions("1.0", "1.0.1") < 0);
    }

    #[test]
    fn test_compare_versions_with_v_prefix() {
        // The function itself doesn't strip 'v', but it handles non-numeric parts as 0
        assert!(compare_versions("1.0.0", "v1.0.0") > 0); // 'v' parses as 0
        assert!(compare_versions("v1.0.0", "1.0.0") < 0);
    }

    #[test]
    fn test_compare_versions_major_minor_patch() {
        assert!(compare_versions("2.0.0", "1.0.0") > 0);
        assert!(compare_versions("1.1.0", "1.0.0") > 0);
        assert!(compare_versions("1.0.1", "1.0.0") > 0);
        assert!(compare_versions("1.0.0", "2.0.0") < 0);
        assert!(compare_versions("1.0.0", "1.1.0") < 0);
        assert!(compare_versions("1.0.0", "1.0.1") < 0);
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
        assert_eq!(deserialized.has_update, true);
        assert_eq!(deserialized.latest_version, "v2.0.0");
        assert_eq!(deserialized.release_notes.unwrap(), "Bug fixes and improvements");
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
