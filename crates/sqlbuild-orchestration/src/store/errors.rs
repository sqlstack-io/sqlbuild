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
    #[error("control-plane operation denied: {reason}; decision {decision_id}")]
    Denied {
        reason: String,
        decision_id: uuid::Uuid,
    },
    #[error("control-store schema is unsupported: {0}")]
    Schema(String),
    #[error("snapshot generation is not newer than the current generation")]
    StaleGeneration,
    #[error("scheduler lease expired or was fenced by another holder")]
    Fenced,
}

impl ControlError {
    /// A store that is briefly busy or unreachable; the same operation may succeed when retried.
    #[must_use]
    pub fn transient(&self) -> bool {
        match self {
            Self::Database(error) => Self::transient_database(error),
            Self::Sqlite(rusqlite::Error::SqliteFailure(error, _)) => matches!(
                error.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            ),
            _ => false,
        }
    }

    pub(crate) fn transient_database(error: &sqlx_core::Error) -> bool {
        match error {
            sqlx_core::Error::Io(_)
            | sqlx_core::Error::PoolTimedOut
            | sqlx_core::Error::WorkerCrashed => true,
            sqlx_core::Error::Database(error) => error.code().is_some_and(|code| {
                code.starts_with("08")
                    || code.starts_with("53")
                    || matches!(code.as_ref(), "57P01" | "57P02" | "57P03")
            }),
            _ => false,
        }
    }
}
