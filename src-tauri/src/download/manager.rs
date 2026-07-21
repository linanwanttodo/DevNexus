use crate::download::chunk::ChunkEngine;
use crate::download::config::DownloadConfig;
use crate::download::progress::{ChunkProgress, DownloadProgress};
use crate::download::storage::DownloadStorage;
use crate::download::task::{ChunkInfo, ChunkStatus, DownloadStatus, DownloadTask};
use chrono::Utc;
use reqwest::header;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tokio::sync::{broadcast, Mutex};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DownloadManagerError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Download error: {0}")]
    DownloadError(String),
}

pub struct DownloadManager {
    tasks: Arc<Mutex<HashMap<String, DownloadTask>>>,
    config: Arc<Mutex<DownloadConfig>>,
    chunk_engine: ChunkEngine,
    storage: Arc<DownloadStorage>,
    progress_tx: broadcast::Sender<DownloadProgress>,
    active_handles: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl DownloadManager {
    pub fn new(config: DownloadConfig, db_path: &str) -> Result<Self, DownloadManagerError> {
        let storage = DownloadStorage::new(db_path)
            .map_err(|e| DownloadManagerError::Storage(e.to_string()))?;
        Self::with_storage(config, storage)
    }

    pub fn with_storage(config: DownloadConfig, storage: DownloadStorage) -> Result<Self, DownloadManagerError> {
        let (progress_tx, _) = broadcast::channel(100);
        let config_mutex = Arc::new(Mutex::new(config.clone()));

        Ok(Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            config: config_mutex,
            chunk_engine: ChunkEngine::new(
                config.max_chunks_per_task,
                config.min_chunk_size,
            ),
            storage: Arc::new(storage),
            progress_tx,
            active_handles: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn subscribe_progress(&self) -> broadcast::Receiver<DownloadProgress> {
        self.progress_tx.subscribe()
    }

    /// 创建下载任务
    pub async fn create_task(&self, url: String, save_path: Option<String>) -> Result<String, DownloadManagerError> {
        let parsed_url = Url::parse(&url)
            .map_err(|e| DownloadManagerError::InvalidUrl(e.to_string()))?;

        let download_url = if self.is_github_url(&url) {
            let config = self.config.lock().await;
            if config.auto_mirror_github {
                config.github_mirrors.iter()
                    .find(|m| m.enabled)
                    .map(|mirror| {
                        if mirror.strip_host {
                            // Xget 风格：https://xget.xi-xu.me/gh/user/repo/...
                            let path = parsed_url.path();
                            format!("{}{}", mirror.url_prefix.trim_end_matches('/'), path)
                        } else {
                            // ghproxy 风格：https://ghproxy.com/https://github.com/...
                            format!("{}{}", mirror.url_prefix, url)
                        }
                    })
                    .unwrap_or(url.clone())
            } else {
                url.clone()
            }
        } else {
            url.clone()
        };

        // 获取文件大小
        let total_size = self.get_file_size(&download_url).await?;

        // 提取文件名
        let filename = self.extract_filename(&parsed_url);

        // 确定保存路径
        let final_path = match save_path {
            Some(path) => format!("{}/{}", path, filename),
            None => {
                let config = self.config.lock().await;
                format!("{}/{}", config.default_save_path, filename)
            }
        };

        // 初始化分块
        let chunks = self.chunk_engine.initialize_chunks(&download_url, total_size).await
            .map_err(|e| DownloadManagerError::DownloadError(e.to_string()))?;

        let task_id = Uuid::new_v4().to_string();
        let task = DownloadTask {
            id: task_id.clone(),
            url: download_url,
            filename,
            save_path: final_path.clone(),
            total_size,
            downloaded_size: 0,
            status: DownloadStatus::Pending,
            chunks,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            speed: 0.0,
            error: None,
        };

        self.storage.save_task(&task)
            .map_err(|e| DownloadManagerError::Storage(e.to_string()))?;

        let mut tasks = self.tasks.lock().await;
        tasks.insert(task_id.clone(), task);

        Ok(task_id)
    }

    fn is_github_url(&self, url: &str) -> bool {
        url.starts_with("https://github.com/")
            || url.starts_with("http://github.com/")
            || url.starts_with("https://api.github.com/")
    }

    /// 开始下载任务
    pub async fn start_task(&self, task_id: &str) -> Result<(), DownloadManagerError> {
        let mut tasks = self.tasks.lock().await;
        let task = tasks.get_mut(task_id)
            .ok_or_else(|| DownloadManagerError::TaskNotFound(task_id.to_string()))?;

        if task.status != DownloadStatus::Pending && task.status != DownloadStatus::Paused {
            return Err(DownloadManagerError::DownloadError(
                "Task is not in pending or paused state".to_string()
            ));
        }

        task.status = DownloadStatus::Downloading;
        task.started_at = Some(Utc::now());
        task.error = None;

        // 保存到数据库
        self.storage.save_task(task)
            .map_err(|e| DownloadManagerError::Storage(e.to_string()))?;

        drop(tasks);

        // 异步执行下载
        let manager_clone = self.clone_for_download();
        let task_clone = {
            let tasks = self.tasks.lock().await;
            tasks.get(task_id).cloned()
        }.ok_or_else(|| DownloadManagerError::TaskNotFound(task_id.to_string()))?;

        let handle = tokio::spawn(async move {
            let clone_id = task_clone.id.clone();
            let result = manager_clone.execute_download(&task_clone).await;
            if let Err(e) = result {
                eprintln!("Download failed for {}: {}", clone_id, e);
                let mut tasks = manager_clone.tasks.lock().await;
                if let Some(task) = tasks.get_mut(&clone_id) {
                    task.status = DownloadStatus::Failed;
                    task.error = Some(e.to_string());
                    let saved = task.clone();
                    drop(tasks);
                    let _ = manager_clone.storage.save_task(&saved);
                }
            }
            let mut handles = manager_clone.active_handles.lock().await;
            handles.remove(&clone_id);
        });

        let mut handles = self.active_handles.lock().await;
        handles.insert(task_id.to_string(), handle);

        Ok(())
    }

    /// 暂停下载任务
    pub async fn pause_task(&self, task_id: &str) -> Result<(), DownloadManagerError> {
        let mut tasks = self.tasks.lock().await;
        let task = tasks.get_mut(task_id)
            .ok_or_else(|| DownloadManagerError::TaskNotFound(task_id.to_string()))?;

        if task.status != DownloadStatus::Downloading {
            return Err(DownloadManagerError::DownloadError(
                "Task is not downloading".to_string()
            ));
        }

        task.status = DownloadStatus::Paused;

        self.storage.save_task(task)
            .map_err(|e| DownloadManagerError::Storage(e.to_string()))?;

        // 中止后台任务
        self.abort_active_task(task_id).await;

        Ok(())
    }

    /// 取消下载任务
    pub async fn cancel_task(&self, task_id: &str) -> Result<(), DownloadManagerError> {
        let mut handles = self.active_handles.lock().await;
        if let Some(handle) = handles.remove(task_id) {
            handle.abort();
        }
        drop(handles);

        let mut tasks = self.tasks.lock().await;
        let task = tasks.get_mut(task_id)
            .ok_or_else(|| DownloadManagerError::TaskNotFound(task_id.to_string()))?;

        task.status = DownloadStatus::Cancelled;

        self.storage.save_task(task)
            .map_err(|e| DownloadManagerError::Storage(e.to_string()))?;

        // 删除临时文件
        Self::cleanup_temp_files(&task.save_path, task.chunks.len()).await;

        Ok(())
    }

    /// 删除下载任务
    pub async fn delete_task(&self, task_id: &str, delete_file: bool) -> Result<(), DownloadManagerError> {
        let tasks = self.tasks.lock().await;
        let task = tasks.get(task_id)
            .ok_or_else(|| DownloadManagerError::TaskNotFound(task_id.to_string()))?;

        if delete_file {
            // 删除下载的文件
            if let Err(e) = tokio::fs::remove_file(&task.save_path).await {
                eprintln!("Failed to delete file: {}", e);
            }
        }

        // 删除临时文件
        Self::cleanup_temp_files(&task.save_path, task.chunks.len()).await;

        // 从数据库删除
        self.storage.delete_task(task_id)
            .map_err(|e| DownloadManagerError::Storage(e.to_string()))?;

        // 从内存删除
        drop(tasks);
        let mut tasks = self.tasks.lock().await;
        tasks.remove(task_id);

        Ok(())
    }

    /// 获取所有任务
    pub async fn get_all_tasks(&self) -> Vec<DownloadTask> {
        let tasks = self.tasks.lock().await;
        tasks.values().cloned().collect()
    }

    /// 获取单个任务
    pub async fn get_task(&self, task_id: &str) -> Option<DownloadTask> {
        let tasks = self.tasks.lock().await;
        tasks.get(task_id).cloned()
    }

    /// 获取或创建配置
    pub async fn get_config(&self) -> DownloadConfig {
        self.config.lock().await.clone()
    }

    /// 更新配置
    pub async fn update_config(&self, config: DownloadConfig) {
        let mut cfg = self.config.lock().await;
        *cfg = config;
    }

    async fn abort_active_task(&self, task_id: &str) {
        let mut handles = self.active_handles.lock().await;
        if let Some(handle) = handles.remove(task_id) {
            handle.abort();
        }
    }

    // ========== 私有方法 ==========

    async fn get_file_size(&self, url: &str) -> Result<u64, DownloadManagerError> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::ACCEPT, header::HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
        ));
        headers.insert(header::ACCEPT_LANGUAGE, header::HeaderValue::from_static("en-US,en;q=0.9,zh-CN;q=0.8"));
        headers.insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-cache"));
        headers.insert(header::UPGRADE_INSECURE_REQUESTS, header::HeaderValue::from_static("1"));
        headers.insert("Sec-Fetch-Dest", header::HeaderValue::from_static("document"));
        headers.insert("Sec-Fetch-Mode", header::HeaderValue::from_static("navigate"));
        headers.insert("Sec-Fetch-Site", header::HeaderValue::from_static("none"));
        headers.insert(header::CONNECTION, header::HeaderValue::from_static("keep-alive"));

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
            .default_headers(headers)
            .build()
            .map_err(DownloadManagerError::Http)?;
        let response = client.head(url).send().await?;

        let size = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        Ok(size)
    }

    fn extract_filename(&self, url: &Url) -> String {
        let path = url.path();

        // 从路径提取文件名
        let filename = path.rsplit('/').next().unwrap_or("download");

        if filename.is_empty() || !filename.contains('.') {
            // 如果无法提取，生成随机文件名
            return format!("download_{}", Uuid::new_v4().simple());
        }

        filename.to_string()
    }

    async fn execute_download(&self, task: &DownloadTask) -> Result<(), DownloadManagerError> {
        let task_id = task.id.clone();
        let start_time = Instant::now();
        let retry_count = {
            let config = self.config.lock().await;
            config.retry_count
        };
        let (max_workers, cookie) = {
            let config = self.config.lock().await;
            let workers = config.max_chunks_per_task.min(task.chunks.len()).max(1);
            (workers, config.cookie_string.clone())
        };

        let save_path_dir = PathBuf::from(&task.save_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        tokio::fs::create_dir_all(&save_path_dir).await?;

        // 全局实时进度计数器（每个分块流式更新此值）
        let global_progress = Arc::new(AtomicU64::new(0));
        let shared_errors: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        // 定时进度推送器（每秒 2 次）
        let progress_mgr = self.clone_for_download();
        let progress_task_id = task_id.clone();
        let progress_global = global_progress.clone();
        let progress_start = start_time;
        let progress_total_size = task.total_size;
        let progress_handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let downloaded = progress_global.load(Ordering::SeqCst);
                if downloaded == 0 {
                    continue;
                }
                // 检查是否所有 worker 都结束了
                {
                    let mut tasks = progress_mgr.tasks.lock().await;
                    if let Some(t) = tasks.get_mut(&progress_task_id) {
                        t.downloaded_size = downloaded;
                        let elapsed = progress_start.elapsed().as_secs_f64();
                        if elapsed > 0.0 {
                            t.speed = downloaded as f64 / elapsed;
                        }
                    }
                }
                progress_mgr.send_progress(&progress_task_id).await;
            }
        });

        // 工作队列
        let queue = Arc::new(Mutex::new(VecDeque::from(task.chunks.clone())));
        let mut worker_handles = Vec::new();

        for _ in 0..max_workers {
            let queue = queue.clone();
            let global_progress = global_progress.clone();
            let shared_errors = shared_errors.clone();
            let task_id = task_id.clone();
            let url = task.url.clone();
            let save_path = task.save_path.clone();
            let chunk_engine = self.chunk_engine.clone();
            let retry_count = retry_count;
            let cookie = cookie.clone();
            let mgr = self.clone_for_download();

            let handle = tokio::spawn(async move {
                loop {
                    {
                        let err = shared_errors.lock().await;
                        if err.is_some() {
                            return;
                        }
                    }

                    let chunk = {
                        let mut q = queue.lock().await;
                        q.pop_front()
                    };

                    let chunk = match chunk {
                        Some(c) => c,
                        None => break,
                    };

                    let temp_path = PathBuf::from(format!("{}.part{}", save_path, chunk.id));
                    let mut last_error = None;
                    let cookie_ref = if cookie.is_empty() { None } else { Some(cookie.as_str()) };

                    for attempt in 0..=retry_count {
                        match chunk_engine
                            .download_chunk(&url, &chunk, &temp_path, global_progress.clone(), cookie_ref)
                            .await
                        {
                            Ok(downloaded) => {
                                {
                                    let mut tasks = mgr.tasks.lock().await;
                                    if let Some(t) = tasks.get_mut(&task_id) {
                                        if let Some(c) = t.chunks.iter_mut().find(|c| c.id == chunk.id) {
                                            c.status = ChunkStatus::Completed;
                                            c.downloaded = downloaded;
                                        }
                                    }
                                }
                                break;
                            }
                            Err(e) => {
                                last_error = Some(e);
                                if attempt < retry_count {
                                    let wait = std::time::Duration::from_secs(1 << attempt);
                                    eprintln!("Chunk {} failed (attempt {}/{}), retrying in {:?}",
                                        chunk.id, attempt + 1, retry_count, wait);
                                    tokio::time::sleep(wait).await;
                                }
                            }
                        }
                    }

                    if let Some(e) = last_error {
                        eprintln!("Chunk {} failed after {} attempts: {}", chunk.id, retry_count, e);
                        let mut err = shared_errors.lock().await;
                        *err = Some(format!("Chunk {}: {}", chunk.id, e));
                        return;
                    }
                }
            });

            worker_handles.push(handle);
        }

        // 等待所有 worker 结束
        for handle in worker_handles {
            let _ = handle.await;
        }

        // 停止进度推送器
        progress_handle.abort();

        // 检查是否有错误
        {
            let err = shared_errors.lock().await;
            if let Some(msg) = err.as_ref() {
                return Err(DownloadManagerError::DownloadError(msg.clone()));
            }
        }

        // 合并分块
        self.chunk_engine.merge_chunks(
            &task.save_path,
            task.chunks.len()
        ).await
        .map_err(|e| DownloadManagerError::DownloadError(e.to_string()))?;

        {
            let mut tasks = self.tasks.lock().await;
            if let Some(t) = tasks.get_mut(&task_id) {
                t.status = DownloadStatus::Completed;
                t.completed_at = Some(Utc::now());
                t.downloaded_size = t.total_size;
                self.storage.save_task(t)
                    .map_err(|e| DownloadManagerError::Storage(e.to_string()))?;
            }
        }

        Ok(())
    }

    async fn send_progress(&self, task_id: &str) {
        let tasks = self.tasks.lock().await;
        let task = match tasks.get(task_id) {
            Some(t) => t.clone(),
            None => return,
        };
        drop(tasks);

        let downloaded = task.downloaded_size;
        let percentage = if task.total_size > 0 {
            (downloaded as f64 / task.total_size as f64) * 100.0
        } else {
            0.0
        };

        let eta = if task.speed > 0.0 {
            let remaining = task.total_size.saturating_sub(downloaded);
            Some((remaining as f64 / task.speed) as u64)
        } else {
            None
        };

        let chunks: Vec<ChunkProgress> = task.chunks.iter().map(|c| ChunkProgress {
            id: c.id,
            start: c.start,
            end: c.end,
            downloaded: c.downloaded,
            status: format!("{}", c.status),
        }).collect();

        let progress = DownloadProgress {
            task_id: task.id.clone(),
            total_size: task.total_size,
            downloaded_size: downloaded,
            speed: task.speed,
            eta_seconds: eta,
            percentage,
            active_chunks: task.chunks.iter()
                .filter(|c| c.status == ChunkStatus::Downloading)
                .count(),
            chunks,
            timestamp: Utc::now(),
        };

        let _ = self.progress_tx.send(progress);
    }

    async fn cleanup_temp_files(save_path: &str, chunk_count: usize) {
        for i in 0..chunk_count {
            let part_path = format!("{}.part{}", save_path, i);
            let _ = tokio::fs::remove_file(part_path).await;
        }
    }

    fn clone_for_download(&self) -> Self {
        Self {
            tasks: self.tasks.clone(),
            config: self.config.clone(),
            chunk_engine: self.chunk_engine.clone(),
            storage: self.storage.clone(),
            progress_tx: self.progress_tx.clone(),
            active_handles: self.active_handles.clone(),
        }
    }

    // Exposed for testing
    #[cfg(test)]
    pub fn storage(&self) -> &Arc<DownloadStorage> {
        &self.storage
    }

    #[cfg(test)]
    pub async fn insert_task_directly(&self, task: DownloadTask) {
        let mut tasks = self.tasks.lock().await;
        self.storage.save_task(&task).unwrap();
        tasks.insert(task.id.clone(), task);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::task::{ChunkInfo, ChunkStatus, DownloadStatus, DownloadTask};
    use chrono::Utc;

    fn make_manager() -> DownloadManager {
        let config = DownloadConfig::default();
        let storage = DownloadStorage::new_in_memory().unwrap();
        DownloadManager::with_storage(config, storage).unwrap()
    }

    fn make_task(id: &str) -> DownloadTask {
        DownloadTask {
            id: id.into(),
            url: "https://example.com/file.zip".into(),
            filename: "file.zip".into(),
            save_path: "/tmp/file.zip".into(),
            total_size: 1000,
            downloaded_size: 0,
            status: DownloadStatus::Pending,
            chunks: vec![ChunkInfo {
                id: 0, start: 0, end: 999, downloaded: 0, status: ChunkStatus::Pending,
            }],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            speed: 0.0,
            error: None,
        }
    }

    #[tokio::test]
    async fn test_create_task_invalid_url() {
        let manager = make_manager();
        let result = manager.create_task("not-a-url".into(), None).await;
        assert!(result.is_err());
        match result {
            Err(DownloadManagerError::InvalidUrl(_)) => {},
            _ => panic!("Expected InvalidUrl error"),
        }
    }

    #[tokio::test]
    async fn test_create_and_get_task() {
        let manager = make_manager();
        let task = make_task("test-1");
        manager.insert_task_directly(task.clone()).await;

        let loaded = manager.get_task("test-1").await.unwrap();
        assert_eq!(loaded.id, "test-1");
        assert_eq!(loaded.status, DownloadStatus::Pending);
    }

    #[tokio::test]
    async fn test_get_nonexistent_task() {
        let manager = make_manager();
        let loaded = manager.get_task("ghost").await;
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_get_all_tasks() {
        let manager = make_manager();
        manager.insert_task_directly(make_task("a")).await;
        manager.insert_task_directly(make_task("b")).await;

        let tasks = manager.get_all_tasks().await;
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn test_start_nonexistent_task() {
        let manager = make_manager();
        let result = manager.start_task("ghost").await;
        match result {
            Err(DownloadManagerError::TaskNotFound(_)) => {},
            other => panic!("Expected TaskNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_pause_nonexistent_task() {
        let manager = make_manager();
        let result = manager.pause_task("ghost").await;
        match result {
            Err(DownloadManagerError::TaskNotFound(_)) => {},
            other => panic!("Expected TaskNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_pause_task_not_downloading() {
        let manager = make_manager();
        let mut task = make_task("idle");
        task.status = DownloadStatus::Pending;
        manager.insert_task_directly(task).await;

        let result = manager.pause_task("idle").await;
        match result {
            Err(DownloadManagerError::DownloadError(_)) => {},
            other => panic!("Expected DownloadError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_task() {
        let manager = make_manager();
        let result = manager.cancel_task("ghost").await;
        match result {
            Err(DownloadManagerError::TaskNotFound(_)) => {},
            other => panic!("Expected TaskNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_delete_nonexistent_task() {
        let manager = make_manager();
        let result = manager.delete_task("ghost", false).await;
        match result {
            Err(DownloadManagerError::TaskNotFound(_)) => {},
            other => panic!("Expected TaskNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_delete_task_removes_from_storage() {
        let manager = make_manager();
        let task = make_task("to-delete");
        manager.insert_task_directly(task).await;

        assert!(manager.get_task("to-delete").await.is_some());

        // Override status to something that won't trigger network
        {
            let mut tasks = manager.tasks.lock().await;
            if let Some(t) = tasks.get_mut("to-delete") {
                t.status = DownloadStatus::Completed;
            }
        }

        manager.delete_task("to-delete", false).await.unwrap();
        assert!(manager.get_task("to-delete").await.is_none());
    }

    #[tokio::test]
    async fn test_config_defaults() {
        let manager = make_manager();
        let config = manager.get_config().await;
        assert_eq!(config.max_concurrent_tasks, 3);
        assert_eq!(config.max_chunks_per_task, 16);
    }

    #[tokio::test]
    async fn test_update_config() {
        let manager = make_manager();
        let mut config = manager.get_config().await;
        config.max_concurrent_tasks = 10;
        config.retry_count = 7;
        manager.update_config(config.clone()).await;

        let updated = manager.get_config().await;
        assert_eq!(updated.max_concurrent_tasks, 10);
        assert_eq!(updated.retry_count, 7);
    }

    #[tokio::test]
    async fn test_subscribe_progress() {
        let manager = make_manager();
        let mut rx = manager.subscribe_progress();
        // Send a progress update via internal channel
        let progress = DownloadProgress {
            task_id: "test".into(),
            total_size: 100,
            downloaded_size: 50,
            speed: 100.0,
            eta_seconds: Some(5),
            percentage: 50.0,
            active_chunks: 1,
            chunks: vec![],
            timestamp: Utc::now(),
        };
        manager.progress_tx.send(progress).unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received.task_id, "test");
        assert_eq!(received.percentage, 50.0);
    }

    #[test]
    fn test_extract_filename_from_url() {
        let config = DownloadConfig::default();
        let db = tempfile::NamedTempFile::new().unwrap();
        let manager = DownloadManager::new(config, db.path().to_str().unwrap()).unwrap();

        let url = Url::parse("https://example.com/files/document.pdf").unwrap();
        let name = manager.extract_filename(&url);
        assert_eq!(name, "document.pdf");
    }

    #[test]
    fn test_extract_filename_no_extension() {
        let config = DownloadConfig::default();
        let db = tempfile::NamedTempFile::new().unwrap();
        let manager = DownloadManager::new(config, db.path().to_str().unwrap()).unwrap();

        let url = Url::parse("https://example.com/download").unwrap();
        let name = manager.extract_filename(&url);
        assert!(name.starts_with("download_"));
    }

    #[test]
    fn test_extract_filename_nested_path() {
        let config = DownloadConfig::default();
        let db = tempfile::NamedTempFile::new().unwrap();
        let manager = DownloadManager::new(config, db.path().to_str().unwrap()).unwrap();

        let url = Url::parse("https://example.com/path/to/file.txt").unwrap();
        let name = manager.extract_filename(&url);
        assert_eq!(name, "file.txt");
    }

    #[tokio::test]
    async fn test_start_task_invalid_state_transition() {
        let manager = make_manager();
        let mut task = make_task("cant-start");
        task.status = DownloadStatus::Completed;
        manager.insert_task_directly(task).await;

        let result = manager.start_task("cant-start").await;
        assert!(result.is_err());
    }
}