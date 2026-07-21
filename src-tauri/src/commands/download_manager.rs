use crate::download::config::GithubMirror;
use crate::download::{DownloadConfig, DownloadManager};
use tauri::State;

#[tauri::command]
pub async fn create_download(
    manager: State<'_, DownloadManager>,
    url: String,
    save_path: Option<String>,
) -> Result<String, String> {
    manager
        .create_task(url, save_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_github_mirrors(
    manager: State<'_, DownloadManager>,
) -> Result<Vec<GithubMirror>, String> {
    let config = manager.get_config().await;
    Ok(config.github_mirrors)
}

#[tauri::command]
pub fn get_changelog(version: Option<String>) -> Result<Option<crate::download::changelog::ChangelogEntry>, String> {
    Ok(match version {
        Some(v) => crate::download::changelog::get_changelog(&v),
        None => crate::download::changelog::get_latest_changelog(),
    })
}

#[tauri::command]
pub async fn save_github_mirrors(
    manager: State<'_, DownloadManager>,
    mirrors: Vec<GithubMirror>,
) -> Result<(), String> {
    let mut config = manager.get_config().await;
    config.github_mirrors = mirrors;
    manager.update_config(config).await;
    Ok(())
}

#[tauri::command]
pub async fn start_download(
    manager: State<'_, DownloadManager>,
    task_id: String,
) -> Result<(), String> {
    manager
        .start_task(&task_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_download(
    manager: State<'_, DownloadManager>,
    task_id: String,
) -> Result<(), String> {
    manager
        .pause_task(&task_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_download(
    manager: State<'_, DownloadManager>,
    task_id: String,
) -> Result<(), String> {
    manager
        .start_task(&task_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_download(
    manager: State<'_, DownloadManager>,
    task_id: String,
) -> Result<(), String> {
    manager
        .cancel_task(&task_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_download(
    manager: State<'_, DownloadManager>,
    task_id: String,
    delete_file: bool,
) -> Result<(), String> {
    manager
        .delete_task(&task_id, delete_file)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_downloads(
    manager: State<'_, DownloadManager>,
) -> Result<Vec<crate::download::DownloadTask>, String> {
    Ok(manager.get_all_tasks().await)
}

#[tauri::command]
pub async fn get_download(
    manager: State<'_, DownloadManager>,
    task_id: String,
) -> Result<Option<crate::download::DownloadTask>, String> {
    Ok(manager.get_task(&task_id).await)
}

#[tauri::command]
pub async fn get_download_config(
    manager: State<'_, DownloadManager>,
) -> Result<DownloadConfig, String> {
    Ok(manager.get_config().await)
}

#[tauri::command]
pub async fn set_download_config(
    manager: State<'_, DownloadManager>,
    config: DownloadConfig,
) -> Result<(), String> {
    manager.update_config(config).await;
    Ok(())
}