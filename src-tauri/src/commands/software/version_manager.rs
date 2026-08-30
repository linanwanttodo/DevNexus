// software/version_manager.rs — Version Detection and Management
//
// Provides version fetching from various sources (GitHub, Node.js dist, Go downloads)
// and version validation utilities.

use serde::{Deserialize, Serialize};

/// Software version information
#[derive(Serialize, Deserialize, Clone)]
pub struct VersionInfo {
    pub version: String,
    pub url: Option<String>,
    pub release_date: Option<String>,
}

/// Safely get version by running command with timeout
pub async fn safe_get_version(cmd: &str, gui_apps: &[&str]) -> String {
    if gui_apps.contains(&cmd) {
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
        Ok(Ok(Ok(output))) => {
            let code = output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string());
            format!("version check failed (exit {})", code)
        }
        Ok(Ok(Err(e))) => format!("version check failed: {}", e),
        Ok(Err(e)) => format!("version check failed: {}", e),
        Err(_) => "timeout".to_string(),
    }
}

/// Fetch versions from GitHub Releases API
#[allow(dead_code)]
async fn fetch_github_versions(owner: &str, repo: &str) -> Result<Vec<String>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=30",
        owner, repo
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
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
        .filter(|v| {
            !v.contains("rc")
                && !v.contains("beta")
                && !v.contains("alpha")
                && !v.contains("nightly")
        })
        .map(|v| v.trim_start_matches('v').to_string())
        .collect();
    if versions.is_empty() {
        Err("No stable releases found".to_string())
    } else {
        Ok(versions)
    }
}

/// Fetch Node.js versions from official dist directory
#[allow(dead_code)]
async fn fetch_node_versions() -> Result<Vec<String>, String> {
    let url = "https://nodejs.org/dist/index.json";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let resp = client
        .get(url)
        .send()
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

/// Fetch Go versions from official download page
#[allow(dead_code)]
async fn fetch_go_versions() -> Result<Vec<String>, String> {
    let url = "https://go.dev/dl/?mode=json";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let resp = client
        .get(url)
        .send()
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

/// Fetch available versions for a specific software package (internal helper)
/// Note: This is a simplified version. The full implementation remains in software_core.rs
pub async fn fetch_software_versions_internal(_package_name: &str) -> Result<Vec<String>, String> {
    Err("Version fetching requires software_data module - use software_core::fetch_software_versions instead".to_string())
}

/// Validate version string format
pub fn is_valid_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// Merge versions from multiple sources (deduplication)
pub fn merge_versions(existing: &mut String, new_version: &str) {
    let cur = existing.trim();
    let new_ver = new_version.trim();
    if cur.is_empty() {
        *existing = new_ver.to_string();
        return;
    }
    let placeholder = ["installed", "unknown", "N/A"];
    let cur_is_placeholder = placeholder.contains(&cur);
    let new_is_placeholder = placeholder.contains(&new_ver);

    if cur == new_ver || new_is_placeholder {
        return;
    }
    if cur_is_placeholder {
        *existing = new_ver.to_string();
        return;
    }
    if !cur.split(',').any(|v| v.trim() == new_ver) {
        existing.push_str(&format!(", {}", new_ver));
    }
}
