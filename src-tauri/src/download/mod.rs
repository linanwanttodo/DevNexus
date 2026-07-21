pub mod manager;
pub mod task;
pub mod chunk;
pub mod progress;
pub mod config;
pub mod storage;
pub mod changelog;

pub use manager::DownloadManager;
pub use task::{DownloadTask, DownloadStatus, ChunkInfo, ChunkStatus};
pub use progress::DownloadProgress;
pub use config::DownloadConfig;
