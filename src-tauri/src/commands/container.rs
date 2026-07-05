use serde::{Deserialize, Serialize};
use std::process::Command;

// ── Data structures ──────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String, // "running" | "exited" | "paused" | "created"
    pub state: String,  // full state string from Docker
    pub ports: String,
    pub created: String,
    pub size: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
    pub created: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetworkInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
}

#[derive(Serialize, Deserialize)]
pub struct DockerStatus {
    pub installed: bool,
    pub version: String,
    pub running: bool,
}

// ── Helpers ──────────────────────────────────────────────────────

/// Run a docker command and return stdout, stderr separately.
fn run_docker(args: &[&str]) -> Result<(String, String), String> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute docker: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(if stderr.is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        });
    }
    Ok((stdout, stderr))
}

/// Parse Docker JSON-lines output into a vector of a given type.
fn parse_json_lines<T: serde::de::DeserializeOwned>(output: &str) -> Vec<T> {
    output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<T>(l).ok())
        .collect()
}

// ── Tauri Commands ───────────────────────────────────────────────

#[tauri::command]
pub fn check_docker() -> DockerStatus {
    let version = match run_docker(&["--version"]) {
        Ok((out, _)) => out.trim().to_string(),
        Err(_) => {
            return DockerStatus {
                installed: false,
                version: String::new(),
                running: false,
            }
        }
    };
    let running = match run_docker(&["info", "--format", "{{.ServerVersion}}"]) {
        Ok((out, _)) => !out.trim().is_empty(),
        Err(_) => false,
    };
    DockerStatus {
        installed: true,
        version,
        running,
    }
}

#[tauri::command]
pub fn list_containers(all: bool) -> Result<Vec<ContainerInfo>, String> {
    let mut args = vec!["ps", "--format", "{{json .}}", "--no-trunc"];
    if all {
        args.push("-a");
    }
    let (stdout, _) = run_docker(&args)?;

    let raw: Vec<serde_json::Value> = parse_json_lines(&stdout);
    let containers = raw
        .iter()
        .map(|v| {
            let id = v
                .get("ID")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let name = v
                .get("Names")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim_start_matches('/')
                .to_string();
            let image = v
                .get("Image")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let state = v
                .get("State")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            // Normalize status to one of: running, exited, paused, created
            let status = match state.as_str() {
                "running" => "running".to_string(),
                "exited" => "exited".to_string(),
                "paused" => "paused".to_string(),
                _ => state.clone(),
            };
            let ports = v
                .get("Ports")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let created = v
                .get("CreatedAt")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let size = v
                .get("Size")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            ContainerInfo {
                id,
                name,
                image,
                status,
                state,
                ports,
                created,
                size,
            }
        })
        .collect();
    Ok(containers)
}

#[tauri::command]
pub fn container_action(name: String, action: String) -> Result<String, String> {
    let (stdout, _) = run_docker(&[&action, &name])?;
    Ok(stdout.trim().to_string())
}

#[tauri::command]
pub fn get_container_logs(name: String, tail: Option<u32>) -> Result<String, String> {
    let tail = tail.unwrap_or(200);
    let tail_str = tail.to_string();
    let (stdout, stderr) = run_docker(&["logs", "--tail", &tail_str, &name])?;
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{}{}", stdout, stderr)
    };
    Ok(combined)
}

#[tauri::command]
pub fn exec_in_container(name: String, command: String) -> Result<String, String> {
    let (stdout, stderr) = run_docker(&["exec", &name, "sh", "-c", &command])?;
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{}{}", stdout, stderr)
    };
    Ok(combined)
}

#[tauri::command]
pub fn list_images() -> Result<Vec<ImageInfo>, String> {
    let (stdout, _) = run_docker(&["images", "--format", "{{json .}}", "--no-trunc"])?;
    let raw: Vec<serde_json::Value> = parse_json_lines(&stdout);
    let images = raw
        .iter()
        .map(|v| {
            let id = v
                .get("ID")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let repository = v
                .get("Repository")
                .and_then(|x| x.as_str())
                .unwrap_or("<none>")
                .to_string();
            let tag = v
                .get("Tag")
                .and_then(|x| x.as_str())
                .unwrap_or("<none>")
                .to_string();
            let size = v
                .get("Size")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let created = v
                .get("CreatedAt")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            ImageInfo {
                id,
                repository,
                tag,
                size,
                created,
            }
        })
        .collect();
    Ok(images)
}

#[tauri::command]
pub fn pull_image(image: String) -> Result<String, String> {
    let (stdout, _) = run_docker(&["pull", &image])?;
    Ok(stdout.trim().to_string())
}

#[tauri::command]
pub fn remove_image(image_id: String, force: bool) -> Result<String, String> {
    let mut args = vec!["rmi"];
    if force {
        args.push("-f");
    }
    args.push(&image_id);
    let (stdout, _) = run_docker(&args)?;
    Ok(stdout.trim().to_string())
}

#[tauri::command]
pub fn build_image(tag: String, path: String) -> Result<String, String> {
    let (stdout, _) = run_docker(&["build", "-t", &tag, &path])?;
    Ok(stdout.trim().to_string())
}

#[tauri::command]
pub fn tag_image(image_id: String, tag: String) -> Result<String, String> {
    let (stdout, _) = run_docker(&["tag", &image_id, &tag])?;
    Ok(stdout.trim().to_string())
}

#[tauri::command]
pub fn push_image(tag: String) -> Result<String, String> {
    let (stdout, _) = run_docker(&["push", &tag])?;
    Ok(stdout.trim().to_string())
}

#[tauri::command]
pub fn list_volumes() -> Result<Vec<VolumeInfo>, String> {
    let (stdout, _) = run_docker(&["volume", "ls", "--format", "{{json .}}"])?;
    let raw: Vec<serde_json::Value> = parse_json_lines(&stdout);
    let volumes = raw
        .iter()
        .map(|v| {
            let name = v
                .get("Name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let driver = v
                .get("Driver")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let mountpoint = v
                .get("Mountpoint")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let created = v
                .get("CreatedAt")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            VolumeInfo {
                name,
                driver,
                mountpoint,
                created,
            }
        })
        .collect();
    Ok(volumes)
}

#[tauri::command]
pub fn volume_action(name: String, action: String) -> Result<String, String> {
    // action: create, rm
    let (stdout, _) = run_docker(&["volume", &action, &name])?;
    Ok(stdout.trim().to_string())
}

#[tauri::command]
pub fn list_networks() -> Result<Vec<NetworkInfo>, String> {
    let (stdout, _) = run_docker(&["network", "ls", "--format", "{{json .}}"])?;
    let raw: Vec<serde_json::Value> = parse_json_lines(&stdout);
    let networks = raw
        .iter()
        .map(|v| {
            let id = v
                .get("ID")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let name = v
                .get("Name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let driver = v
                .get("Driver")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let scope = v
                .get("Scope")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            NetworkInfo {
                id,
                name,
                driver,
                scope,
            }
        })
        .collect();
    Ok(networks)
}

#[tauri::command]
pub fn network_action(name: String, action: String) -> Result<String, String> {
    // action: create, rm
    let (stdout, _) = run_docker(&["network", &action, &name])?;
    Ok(stdout.trim().to_string())
}

#[tauri::command]
pub fn compose_up(file: Option<String>, project_name: Option<String>) -> Result<String, String> {
    let mut args = vec!["compose"];
    if let Some(f) = &file {
        args.push("-f");
        args.push(f);
    }
    if let Some(p) = &project_name {
        args.push("-p");
        args.push(p);
    }
    args.push("up");
    args.push("-d");
    let (stdout, _) = run_docker(&args)?;
    Ok(stdout.trim().to_string())
}

#[tauri::command]
pub fn compose_down(file: Option<String>, project_name: Option<String>) -> Result<String, String> {
    let mut args = vec!["compose"];
    if let Some(f) = &file {
        args.push("-f");
        args.push(f);
    }
    if let Some(p) = &project_name {
        args.push("-p");
        args.push(p);
    }
    args.push("down");
    let (stdout, _) = run_docker(&args)?;
    Ok(stdout.trim().to_string())
}

#[tauri::command]
pub fn compose_ps(
    file: Option<String>,
    project_name: Option<String>,
) -> Result<Vec<ContainerInfo>, String> {
    let mut args = vec!["compose"];
    if let Some(f) = &file {
        args.push("-f");
        args.push(f);
    }
    if let Some(p) = &project_name {
        args.push("-p");
        args.push(p);
    }
    args.push("ps");
    args.push("--format");
    args.push("{{json .}}");
    let (stdout, _) = run_docker(&args)?;
    let raw: Vec<serde_json::Value> = parse_json_lines(&stdout);
    let containers = raw
        .iter()
        .map(|v| {
            let id = v
                .get("ID")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let name = v
                .get("Name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let image = v
                .get("Image")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let state = v
                .get("State")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let status = match state.as_str() {
                "running" => "running".to_string(),
                "exited" => "exited".to_string(),
                "paused" => "paused".to_string(),
                _ => state.clone(),
            };
            let ports = v
                .get("Ports")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let created = String::new();
            let size = String::new();
            ContainerInfo {
                id,
                name,
                image,
                status,
                state,
                ports,
                created,
                size,
            }
        })
        .collect();
    Ok(containers)
}

#[tauri::command]
pub fn compose_logs(
    file: Option<String>,
    project_name: Option<String>,
    tail: Option<u32>,
) -> Result<String, String> {
    let mut args = vec!["compose"];
    if let Some(f) = &file {
        args.push("-f");
        args.push(f);
    }
    if let Some(p) = &project_name {
        args.push("-p");
        args.push(p);
    }
    args.push("logs");
    let tail = tail.unwrap_or(100);
    let tail_str = tail.to_string();
    args.push("--tail");
    args.push(&tail_str);
    let (stdout, stderr) = run_docker(&args)?;
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{}{}", stdout, stderr)
    };
    Ok(combined)
}
