use crate::download::task::DownloadTask;
use rusqlite::params;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Task not found: {0}")]
    NotFound(String),
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        StorageError::Serialization(e.to_string())
    }
}

fn parse_chunks_json(
    chunks_json: &str,
) -> Result<Vec<crate::download::task::ChunkInfo>, rusqlite::Error> {
    serde_json::from_str(chunks_json)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

fn parse_status(status_str: &str) -> crate::download::task::DownloadStatus {
    match status_str {
        "Pending" => crate::download::task::DownloadStatus::Pending,
        "Downloading" => crate::download::task::DownloadStatus::Downloading,
        "Paused" => crate::download::task::DownloadStatus::Paused,
        "Completed" => crate::download::task::DownloadStatus::Completed,
        "Failed" => crate::download::task::DownloadStatus::Failed,
        "Cancelled" => crate::download::task::DownloadStatus::Cancelled,
        _ => crate::download::task::DownloadStatus::Pending,
    }
}

#[derive(Clone)]
pub struct DownloadStorage {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl DownloadStorage {
    pub fn new(db_path: &str) -> Result<Self, StorageError> {
        let conn = rusqlite::Connection::open(db_path)?;

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        storage.create_tables()?;
        Ok(storage)
    }

    pub fn new_in_memory() -> Result<Self, StorageError> {
        let conn = rusqlite::Connection::open_in_memory()?;

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        storage.create_tables()?;
        Ok(storage)
    }

    fn create_tables(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS downloads (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                filename TEXT NOT NULL,
                save_path TEXT NOT NULL,
                total_size INTEGER NOT NULL,
                downloaded_size INTEGER NOT NULL,
                status TEXT NOT NULL,
                chunks_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                speed REAL NOT NULL,
                error TEXT
            )
            "#,
            [],
        )?;
        Ok(())
    }

    pub fn save_task(&self, task: &DownloadTask) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        let chunks_json = serde_json::to_string(&task.chunks)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        conn.execute(
            r#"
            INSERT OR REPLACE INTO downloads 
            (id, url, filename, save_path, total_size, downloaded_size, status, chunks_json, created_at, started_at, completed_at, speed, error)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                task.id,
                task.url,
                task.filename,
                task.save_path,
                task.total_size,
                task.downloaded_size,
                format!("{:?}", task.status),
                chunks_json,
                task.created_at.to_rfc3339(),
                task.started_at.map(|t| t.to_rfc3339()),
                task.completed_at.map(|t| t.to_rfc3339()),
                task.speed,
                task.error.as_deref(),
            ],
        )?;

        Ok(())
    }

    fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<DownloadTask> {
        let status_str: String = row.get(6)?;
        let chunks_json: String = row.get(7)?;
        let chunks: Vec<crate::download::task::ChunkInfo> = parse_chunks_json(&chunks_json)?;

        Ok(DownloadTask {
            id: row.get(0)?,
            url: row.get(1)?,
            filename: row.get(2)?,
            save_path: row.get(3)?,
            total_size: row.get(4)?,
            downloaded_size: row.get(5)?,
            status: parse_status(&status_str),
            chunks,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                .unwrap()
                .with_timezone(&chrono::Utc),
            started_at: row.get::<_, Option<String>>(9)?.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            }),
            completed_at: row.get::<_, Option<String>>(10)?.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            }),
            speed: row.get(11)?,
            error: row.get(12)?,
        })
    }

    pub fn get_task(&self, id: &str) -> Result<Option<DownloadTask>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, filename, save_path, total_size, downloaded_size, status, chunks_json, created_at, started_at, completed_at, speed, error FROM downloads WHERE id = ?1"
        )?;

        let tasks = stmt.query_map(params![id], Self::row_to_task)?;

        let mut result = Vec::new();
        for task in tasks {
            result.push(task?);
        }
        Ok(result.into_iter().next())
    }

    pub fn list_tasks(&self) -> Result<Vec<DownloadTask>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, filename, save_path, total_size, downloaded_size, status, chunks_json, created_at, started_at, completed_at, speed, error FROM downloads ORDER BY created_at DESC"
        )?;

        let tasks = stmt.query_map(params![], Self::row_to_task)?;

        let mut result = Vec::new();
        for task in tasks {
            result.push(task?);
        }
        Ok(result)
    }

    pub fn delete_task(&self, id: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM downloads WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn conn(&self) -> &Arc<Mutex<rusqlite::Connection>> {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::task::{ChunkInfo, ChunkStatus, DownloadStatus, DownloadTask};
    use chrono::Utc;

    fn make_task(id: &str) -> DownloadTask {
        DownloadTask {
            id: id.into(),
            url: format!("https://example.com/{}", id),
            filename: format!("{}.zip", id),
            save_path: format!("/tmp/{}.zip", id),
            total_size: 1000,
            downloaded_size: 0,
            status: DownloadStatus::Pending,
            chunks: vec![ChunkInfo {
                id: 0,
                start: 0,
                end: 999,
                downloaded: 0,
                status: ChunkStatus::Pending,
            }],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            speed: 0.0,
            error: None,
        }
    }

    fn make_storage() -> DownloadStorage {
        DownloadStorage::new_in_memory().unwrap()
    }

    #[test]
    fn test_new_storage_creates_tables() {
        let storage = make_storage();
        let conn = storage.conn().lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='downloads'")
            .unwrap();
        let exists: bool = stmt.exists(params![]).unwrap();
        assert!(exists);
    }

    #[test]
    fn test_save_and_get_task() {
        let storage = make_storage();
        let task = make_task("test-1");
        storage.save_task(&task).unwrap();

        let loaded = storage.get_task("test-1").unwrap().unwrap();
        assert_eq!(loaded.id, "test-1");
        assert_eq!(loaded.url, "https://example.com/test-1");
        assert_eq!(loaded.total_size, 1000);
        assert_eq!(loaded.status, DownloadStatus::Pending);
    }

    #[test]
    fn test_get_nonexistent_task() {
        let storage = make_storage();
        let loaded = storage.get_task("nonexistent").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_save_and_list_tasks() {
        let storage = make_storage();
        storage.save_task(&make_task("a")).unwrap();
        storage.save_task(&make_task("b")).unwrap();
        storage.save_task(&make_task("c")).unwrap();

        let tasks = storage.list_tasks().unwrap();
        assert_eq!(tasks.len(), 3);
    }

    #[test]
    fn test_delete_task() {
        let storage = make_storage();
        storage.save_task(&make_task("delete-me")).unwrap();
        assert!(storage.get_task("delete-me").unwrap().is_some());

        storage.delete_task("delete-me").unwrap();
        assert!(storage.get_task("delete-me").unwrap().is_none());
    }

    #[test]
    fn test_update_task() {
        let storage = make_storage();
        let mut task = make_task("updatable");
        storage.save_task(&task).unwrap();

        task.status = DownloadStatus::Downloading;
        task.downloaded_size = 500;
        task.speed = 1024.0;
        storage.save_task(&task).unwrap();

        let loaded = storage.get_task("updatable").unwrap().unwrap();
        assert_eq!(loaded.status, DownloadStatus::Downloading);
        assert_eq!(loaded.downloaded_size, 500);
        assert_eq!(loaded.speed, 1024.0);
    }

    #[test]
    fn test_task_with_chunks_persistence() {
        let storage = make_storage();
        let mut task = make_task("chunked");
        task.chunks = vec![
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
        task.status = DownloadStatus::Paused;
        task.downloaded_size = 500;
        storage.save_task(&task).unwrap();

        let loaded = storage.get_task("chunked").unwrap().unwrap();
        assert_eq!(loaded.status, DownloadStatus::Paused);
        assert_eq!(loaded.chunks.len(), 2);
        assert_eq!(loaded.chunks[0].status, ChunkStatus::Completed);
        assert_eq!(loaded.chunks[0].downloaded, 500);
        assert_eq!(loaded.chunks[1].status, ChunkStatus::Pending);
    }

    #[test]
    fn test_task_with_error() {
        let storage = make_storage();
        let mut task = make_task("error-task");
        task.status = DownloadStatus::Failed;
        task.error = Some("Connection timeout".into());
        storage.save_task(&task).unwrap();

        let loaded = storage.get_task("error-task").unwrap().unwrap();
        assert_eq!(loaded.status, DownloadStatus::Failed);
        assert_eq!(loaded.error.unwrap(), "Connection timeout");
    }

    #[test]
    fn test_task_with_timestamps() {
        let storage = make_storage();
        let mut task = make_task("timed");
        task.started_at = Some(Utc::now());
        task.completed_at = Some(Utc::now());
        storage.save_task(&task).unwrap();

        let loaded = storage.get_task("timed").unwrap().unwrap();
        assert!(loaded.started_at.is_some());
        assert!(loaded.completed_at.is_some());
    }

    #[test]
    fn test_list_returns_latest_first() {
        let storage = make_storage();
        let mut t1 = make_task("first");
        t1.created_at = chrono::DateTime::from_timestamp(1000, 0).unwrap();
        let mut t2 = make_task("second");
        t2.created_at = chrono::DateTime::from_timestamp(2000, 0).unwrap();

        storage.save_task(&t1).unwrap();
        storage.save_task(&t2).unwrap();

        let tasks = storage.list_tasks().unwrap();
        assert_eq!(tasks[0].id, "second");
        assert_eq!(tasks[1].id, "first");
    }

    #[test]
    fn test_delete_nonexistent() {
        let storage = make_storage();
        storage.delete_task("ghost").unwrap();
    }

    #[test]
    fn test_parse_status_roundtrip() {
        for status in &[
            "Pending",
            "Downloading",
            "Paused",
            "Completed",
            "Failed",
            "Cancelled",
        ] {
            let parsed = parse_status(status);
            let formatted = format!("{:?}", parsed);
            assert!(formatted == *status || format!("{:?}", parsed) == *status);
        }
    }

    #[test]
    fn test_parse_status_invalid_defaults_to_pending() {
        let parsed = parse_status("UnknownStatus");
        assert_eq!(parsed, DownloadStatus::Pending);
    }
}
