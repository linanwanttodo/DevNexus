use crate::download::task::{ChunkInfo, ChunkStatus};
use futures_util::StreamExt;
use reqwest::header;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChunkError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Chunk download failed: {0}")]
    DownloadFailed(String),
}

#[derive(Debug, Clone)]
pub struct ChunkEngine {
    client: reqwest::Client,
    max_chunks: usize,
    min_chunk_size: u64,
}

impl ChunkEngine {
    pub fn new(max_chunks: usize, min_chunk_size: u64) -> Self {
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
        headers.insert("Sec-Fetch-User", header::HeaderValue::from_static("?1"));
        headers.insert(header::CONNECTION, header::HeaderValue::from_static("keep-alive"));

        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
                .default_headers(headers)
                .build()
                .expect("Failed to create HTTP client"),
            max_chunks,
            min_chunk_size,
        }
    }

    /// 检查服务器是否支持Range请求
    async fn check_range_support(&self, url: &str) -> Result<bool, ChunkError> {
        let response = self.client.head(url).send().await?;
        let accepts_ranges = response.headers().get("Accept-Ranges").is_some()
            || response.headers().get("Content-Range").is_some();
        // 确保消耗掉 HEAD 响应体
        drop(response);
        Ok(accepts_ranges)
    }

    /// 计算最优分块数（IDM 风格：小分块 + 多线程均衡）
    fn calculate_optimal_chunks(&self, total_size: u64) -> usize {
        if total_size < self.min_chunk_size {
            return 1;
        }

        // 目标每块 ~2MB，最多 max_chunks 块
        let target_chunk_size: u64 = 2 * 1024 * 1024; // 2MB
        let optimal = (total_size / target_chunk_size) as usize;
        optimal.clamp(1, self.max_chunks)
    }

    /// 初始化分块策略
    pub async fn initialize_chunks(&self, url: &str, total_size: u64) -> Result<Vec<ChunkInfo>, ChunkError> {
        let supports_range = self.check_range_support(url).await.unwrap_or(false);

        if !supports_range || total_size < self.min_chunk_size {
            return Ok(vec![ChunkInfo {
                id: 0,
                start: 0,
                end: total_size.saturating_sub(1),
                downloaded: 0,
                status: ChunkStatus::Pending,
            }]);
        }

        let chunk_count = self.calculate_optimal_chunks(total_size);
        let chunk_size = total_size / chunk_count as u64;

        let mut chunks = Vec::with_capacity(chunk_count);
        for i in 0..chunk_count {
            let start = i as u64 * chunk_size;
            let end = if i == chunk_count - 1 {
                total_size.saturating_sub(1)
            } else {
                start + chunk_size.saturating_sub(1)
            };

            chunks.push(ChunkInfo {
                id: i,
                start,
                end,
                downloaded: 0,
                status: ChunkStatus::Pending,
            });
        }

        Ok(chunks)
    }

    /// 下载单个分块（流式 + 全局实时进度）
    pub async fn download_chunk(
        &self,
        url: &str,
        chunk: &ChunkInfo,
        temp_path: &PathBuf,
        global_progress: Arc<AtomicU64>,
        cookie: Option<&str>,
    ) -> Result<u64, ChunkError> {
        let range_header = format!("bytes={}-{}", chunk.start, chunk.end);

        let mut request = self.client.get(url).header("Range", range_header);
        
        if let Some(c) = cookie {
            request = request.header(header::COOKIE, c);
        }
        
        if chunk.downloaded > 0 {
            let resume_range = format!("bytes={}-{}", chunk.start + chunk.downloaded, chunk.end);
            request = request.header("Range", resume_range);
        }

        let response = request.send().await?;
        
        if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(ChunkError::DownloadFailed(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let mut file = if chunk.downloaded > 0 {
            tokio::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(temp_path)
                .await?
        } else {
            tokio::fs::File::create(temp_path).await?
        };
        let mut total_downloaded = chunk.downloaded;

        let mut stream = response.bytes_stream();
        while let Some(batch) = stream.next().await {
            let batch = batch?;
            file.write_all(&batch).await?;
            total_downloaded += batch.len() as u64;
            global_progress.fetch_add(batch.len() as u64, Ordering::SeqCst);
        }

        Ok(total_downloaded)
    }

    /// 合并所有分块文件
    pub async fn merge_chunks(&self, save_path: &str, chunk_count: usize) -> Result<(), ChunkError> {
        let mut final_file = tokio::fs::File::create(save_path).await?;

        for i in 0..chunk_count {
            let part_path = format!("{}.part{}", save_path, i);
            let part_path = PathBuf::from(&part_path);
            
            if tokio::fs::try_exists(&part_path).await.unwrap_or(false) {
                let mut part_file = tokio::fs::File::open(&part_path).await?;
                let bytes_copied = tokio::io::copy(&mut part_file, &mut final_file).await?;
                if bytes_copied > 0 {
                    final_file.flush().await?;
                }
                tokio::fs::remove_file(&part_path).await?;
            }
        }

        final_file.sync_all().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_optimal_chunks_small_file() {
        let engine = ChunkEngine::new(64, 1_000_000);
        assert_eq!(engine.calculate_optimal_chunks(500_000), 1);
    }

    #[test]
    fn test_calculate_optimal_chunks_10mb() {
        let engine = ChunkEngine::new(64, 1_000_000);
        // 10MB / 2MB = 5
        assert_eq!(engine.calculate_optimal_chunks(10_485_760), 5);
    }

    #[test]
    fn test_calculate_optimal_chunks_50mb() {
        let engine = ChunkEngine::new(64, 1_000_000);
        // 50MB / 2MB = 25
        assert_eq!(engine.calculate_optimal_chunks(52_428_800), 25);
    }

    #[test]
    fn test_calculate_optimal_chunks_large_1gb() {
        let engine = ChunkEngine::new(64, 1_000_000);
        assert_eq!(engine.calculate_optimal_chunks(1_073_741_824), 64);
    }

    #[test]
    fn test_calculate_optimal_chunks_max_chunks() {
        let engine = ChunkEngine::new(16, 1_000_000);
        assert_eq!(engine.calculate_optimal_chunks(100_000_000), 16);
    }

    #[test]
    fn test_calculate_optimal_chunks_zero_size() {
        let engine = ChunkEngine::new(64, 1_000_000);
        assert_eq!(engine.calculate_optimal_chunks(0), 1);
    }

    #[tokio::test]
    async fn test_merge_chunks_single_part() {
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("test_single.bin");
        let part0 = format!("{}.part0", save_path.display());
        tokio::fs::write(&part0, b"hello world").await.unwrap();

        let engine = ChunkEngine::new(8, 1_000_000);
        engine.merge_chunks(save_path.to_str().unwrap(), 1).await.unwrap();

        let content = tokio::fs::read_to_string(&save_path).await.unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_merge_chunks_multiple_parts() {
        let dir = tempfile::tempdir().unwrap();
        let save_path = dir.path().join("test_multi.bin");
        tokio::fs::write(format!("{}.part0", save_path.display()), b"AAA").await.unwrap();
        tokio::fs::write(format!("{}.part1", save_path.display()), b"BBB").await.unwrap();
        tokio::fs::write(format!("{}.part2", save_path.display()), b"CCC").await.unwrap();

        let engine = ChunkEngine::new(8, 1_000_000);
        engine.merge_chunks(save_path.to_str().unwrap(), 3).await.unwrap();

        let content = tokio::fs::read_to_string(&save_path).await.unwrap();
        assert_eq!(content, "AAABBBCCC");
    }
}