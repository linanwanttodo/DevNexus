pub mod changelog;
pub mod chunk;
pub mod config;
pub mod manager;
pub mod progress;
pub mod storage;
pub mod task;

pub use config::DownloadConfig;
pub use manager::DownloadManager;
pub use progress::DownloadProgress;
pub use task::{ChunkInfo, ChunkStatus, DownloadStatus, DownloadTask};
