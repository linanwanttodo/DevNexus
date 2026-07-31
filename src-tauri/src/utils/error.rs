use thiserror::Error;

/// 统一应用错误类型：替代裸字符串 / 哨兵值作为错误信号
#[derive(Debug, Error)]
pub enum DevNexusError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("operation failed: {0}")]
    Operation(String),
    #[error("permission denied: {0}")]
    Permission(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<DevNexusError> for String {
    fn from(e: DevNexusError) -> Self {
        e.to_string()
    }
}
