use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkProgress {
    pub id: usize,
    pub start: u64,
    pub end: u64,
    pub downloaded: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub task_id: String,
    pub total_size: u64,
    pub downloaded_size: u64,
    pub speed: f64,
    pub eta_seconds: Option<u64>,
    pub percentage: f64,
    pub active_chunks: usize,
    pub chunks: Vec<ChunkProgress>,
    pub timestamp: DateTime<Utc>,
}

impl DownloadProgress {
    pub fn format_speed(speed: f64) -> String {
        if speed < 1024.0 {
            format!("{:.1} B/s", speed)
        } else if speed < 1024.0 * 1024.0 {
            format!("{:.1} KB/s", speed / 1024.0)
        } else if speed < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} MB/s", speed / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB/s", speed / (1024.0 * 1024.0 * 1024.0))
        }
    }

    pub fn format_size(size: u64) -> String {
        let size_f = size as f64;
        if size_f < 1024.0 {
            format!("{:.1} B", size_f)
        } else if size_f < 1024.0 * 1024.0 {
            format!("{:.1} KB", size_f / 1024.0)
        } else if size_f < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.1} MB", size_f / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", size_f / (1024.0 * 1024.0 * 1024.0))
        }
    }

    pub fn format_eta(seconds: u64) -> String {
        if seconds < 60 {
            format!("{}s", seconds)
        } else if seconds < 3600 {
            let mins = seconds / 60;
            let secs = seconds % 60;
            format!("{}m {}s", mins, secs)
        } else {
            let hours = seconds / 3600;
            let mins = (seconds % 3600) / 60;
            format!("{}h {}m", hours, mins)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_speed_zero() {
        assert_eq!(DownloadProgress::format_speed(0.0), "0.0 B/s");
    }

    #[test]
    fn test_format_speed_bytes() {
        assert_eq!(DownloadProgress::format_speed(500.0), "500.0 B/s");
    }

    #[test]
    fn test_format_speed_kilobytes() {
        let result = DownloadProgress::format_speed(2048.0);
        assert_eq!(result, "2.0 KB/s");
    }

    #[test]
    fn test_format_speed_megabytes() {
        let result = DownloadProgress::format_speed(5_242_880.0);
        assert_eq!(result, "5.0 MB/s");
    }

    #[test]
    fn test_format_speed_gigabytes() {
        let result = DownloadProgress::format_speed(5_368_709_120.0);
        assert_eq!(result, "5.0 GB/s");
    }

    #[test]
    fn test_format_size_zero() {
        assert_eq!(DownloadProgress::format_size(0), "0.0 B");
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(DownloadProgress::format_size(999), "999.0 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(DownloadProgress::format_size(1536), "1.5 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(DownloadProgress::format_size(10_485_760), "10.0 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(DownloadProgress::format_size(2_147_483_648), "2.0 GB");
    }

    #[test]
    fn test_format_eta_zero() {
        assert_eq!(DownloadProgress::format_eta(0), "0s");
    }

    #[test]
    fn test_format_eta_seconds() {
        assert_eq!(DownloadProgress::format_eta(45), "45s");
    }

    #[test]
    fn test_format_eta_minutes() {
        assert_eq!(DownloadProgress::format_eta(125), "2m 5s");
    }

    #[test]
    fn test_format_eta_hours() {
        assert_eq!(DownloadProgress::format_eta(3661), "1h 1m");
    }

    #[test]
    fn test_format_eta_exact_hour() {
        assert_eq!(DownloadProgress::format_eta(7200), "2h 0m");
    }

    #[test]
    fn test_progress_serde() {
        let progress = DownloadProgress {
            task_id: "test".into(),
            total_size: 1000,
            downloaded_size: 500,
            speed: 1024.0,
            eta_seconds: Some(30),
            percentage: 50.0,
            active_chunks: 3,
            chunks: vec![
                ChunkProgress {
                    id: 0,
                    start: 0,
                    end: 499,
                    downloaded: 500,
                    status: "Completed".into(),
                },
                ChunkProgress {
                    id: 1,
                    start: 500,
                    end: 999,
                    downloaded: 0,
                    status: "Pending".into(),
                },
            ],
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&progress).unwrap();
        let deserialized: DownloadProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.task_id, "test");
        assert_eq!(deserialized.percentage, 50.0);
        assert_eq!(deserialized.active_chunks, 3);
        assert_eq!(deserialized.chunks.len(), 2);
    }
}
