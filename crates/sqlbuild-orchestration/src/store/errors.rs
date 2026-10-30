//! Control-store failures; callers must not publish a successful state after one.

/// A rejected control-plane operation or failed durable write.
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("control store: {0}")]
    Database(#[from] sqlx_core::Error),
    #[error("control store: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("control-store worker failed: {0}")]
    Worker(#[from] tokio::task::JoinError),
    #[error("control-store filesystem: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid control-plane data: {0}")]
    Invalid(String),
    #[error("control-store schema is unsupported: {0}")]
    Schema(String),
    #[error("snapshot generation is not newer than the current generation")]
    StaleGeneration,
    #[error("scheduler lease expired or was fenced by another holder")]
    Fenced,
}
