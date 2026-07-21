use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadStatus::Pending => write!(f, "Pending"),
            DownloadStatus::Downloading => write!(f, "Downloading"),
            DownloadStatus::Paused => write!(f, "Paused"),
            DownloadStatus::Completed => write!(f, "Completed"),
            DownloadStatus::Failed => write!(f, "Failed"),
            DownloadStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub save_path: String,
    pub total_size: u64,
    pub downloaded_size: u64,
    pub status: DownloadStatus,
    pub chunks: Vec<ChunkInfo>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub speed: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub id: usize,
    pub start: u64,
    pub end: u64,
    pub downloaded: u64,
    pub status: ChunkStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChunkStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
}

impl fmt::Display for ChunkStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChunkStatus::Pending => write!(f, "Pending"),
            ChunkStatus::Downloading => write!(f, "Downloading"),
            ChunkStatus::Completed => write!(f, "Completed"),
            ChunkStatus::Failed => write!(f, "Failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_status_display() {
        assert_eq!(DownloadStatus::Pending.to_string(), "Pending");
        assert_eq!(DownloadStatus::Downloading.to_string(), "Downloading");
        assert_eq!(DownloadStatus::Paused.to_string(), "Paused");
        assert_eq!(DownloadStatus::Completed.to_string(), "Completed");
        assert_eq!(DownloadStatus::Failed.to_string(), "Failed");
        assert_eq!(DownloadStatus::Cancelled.to_string(), "Cancelled");
    }

    #[test]
    fn test_chunk_status_partial_eq() {
        assert_eq!(ChunkStatus::Pending, ChunkStatus::Pending);
        assert_ne!(ChunkStatus::Pending, ChunkStatus::Completed);
    }

    #[test]
    fn test_download_task_default_chunks() {
        let task = DownloadTask {
            id: "test-id".into(),
            url: "https://example.com/file.zip".into(),
            filename: "file.zip".into(),
            save_path: "/tmp/file.zip".into(),
            total_size: 1000,
            downloaded_size: 0,
            status: DownloadStatus::Pending,
            chunks: vec![],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            speed: 0.0,
            error: None,
        };
        assert_eq!(task.chunks.len(), 0);
        assert_eq!(task.downloaded_size, 0);
        assert!(task.started_at.is_none());
        assert!(task.completed_at.is_none());
        assert!(task.error.is_none());
    }

    #[test]
    fn test_chunk_info_new() {
        let chunk = ChunkInfo {
            id: 1,
            start: 0,
            end: 999,
            downloaded: 0,
            status: ChunkStatus::Pending,
        };
        assert_eq!(chunk.id, 1);
        assert_eq!(chunk.end - chunk.start + 1, 1000);
    }

    #[test]
    fn test_download_status_serialization() {
        let status = DownloadStatus::Downloading;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Downloading\"");
        let deserialized: DownloadStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, DownloadStatus::Downloading);
    }

    #[test]
    fn test_chunk_status_serialization() {
        let status = ChunkStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: ChunkStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ChunkStatus::Completed);
    }

    #[test]
    fn test_task_serde_roundtrip() {
        let chunks = vec![
            ChunkInfo {
                id: 0,
                start: 0,
                end: 499,
                downloaded: 500,
                status: ChunkStatus::Completed,
            },
            ChunkInfo {
                id: 1,
                start: 500,
                end: 999,
                downloaded: 0,
                status: ChunkStatus::Pending,
            },
        ];
        let task = DownloadTask {
            id: "serde-test".into(),
            url: "https://example.com/test.bin".into(),
            filename: "test.bin".into(),
            save_path: "/tmp/test.bin".into(),
            total_size: 1000,
            downloaded_size: 500,
            status: DownloadStatus::Paused,
            chunks,
            created_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: None,
            speed: 1024.5,
            error: None,
        };
        let json = serde_json::to_string(&task).unwrap();
        let deserialized: DownloadTask = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, task.id);
        assert_eq!(deserialized.total_size, task.total_size);
        assert_eq!(deserialized.downloaded_size, task.downloaded_size);
        assert_eq!(deserialized.status, task.status);
        assert_eq!(deserialized.chunks.len(), 2);
        assert!(deserialized.started_at.is_some());
        assert!(deserialized.completed_at.is_none());
    }
}
