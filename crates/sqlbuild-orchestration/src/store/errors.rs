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
    #[error("run output store: {0}")]
    OutputStore(#[from] object_store::Error),
    #[error("invalid control-plane data: {0}")]
    Invalid(String),
    #[error("control-plane operation denied: {reason}; decision {decision_id}")]
    Denied {
        reason: String,
        decision_id: uuid::Uuid,
    },
    #[error("control-store schema is unsupported: {0}")]
    Schema(String),
    /// The location isn't checking in, so work asked of it would wait for nothing; the message says since when.
    #[error("{0}")]
    NotCheckingIn(String),
    #[error("snapshot generation is not newer than the current generation")]
    StaleGeneration,
    #[error("scheduler lease expired or was fenced by another holder")]
    Fenced,
    /// A pinned snapshot's documents no longer decode, such as after a breaking change to their shape; only the run, trigger or request that reads it fails, with a message that says which.
    #[error("{}", ControlError::unreadable_message(run_id.as_ref()))]
    UnreadableSnapshot {
        snapshot_id: uuid::Uuid,
        /// The run that read it, when the reader knows.
        run_id: Option<uuid::Uuid>,
        /// What failed to decode, for logs.
        detail: String,
    },
}

impl ControlError {
    /// What people read for an unreadable snapshot: a run's own message when it names one, otherwise one for the request.
    fn unreadable_message(run_id: Option<&uuid::Uuid>) -> &'static str {
        match run_id {
            Some(_) => crate::store::constants::UNREADABLE_SNAPSHOT,
            None => crate::store::constants::UNREADABLE_PUBLICATION,
        }
    }

    /// This error, naming `run_id` as the run whose snapshot is unreadable; other errors are unchanged.
    #[must_use]
    pub fn for_run(self, run_id: uuid::Uuid) -> Self {
        match self {
            Self::UnreadableSnapshot {
                snapshot_id,
                detail,
                ..
            } => Self::UnreadableSnapshot {
                snapshot_id,
                run_id: Some(run_id),
                detail,
            },
            error => error,
        }
    }

    /// The control store itself failed or the scheduler lost its lease, so no other trigger can carry on either.
    #[must_use]
    pub fn ends_cycle(&self) -> bool {
        matches!(
            self,
            Self::Database(_)
                | Self::Sqlite(_)
                | Self::Worker(_)
                | Self::Io(_)
                | Self::Schema(_)
                | Self::Fenced
        )
    }

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
