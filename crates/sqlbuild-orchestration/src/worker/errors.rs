//! Transport failures are distinct from rejected pinned execution inputs.

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("invalid worker connection: {0}")]
    Configuration(&'static str),
    #[error("worker API request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("worker API rejected the request with HTTP {0}")]
    Rejected(reqwest::StatusCode),
    /// A server only refuses the shape of a worker's own report when it is older than the worker.
    #[error(
        "the control server is older than this worker and refused its run report; upgrade the server before its workers"
    )]
    OlderServer,
    #[error("worker heartbeat could not start: {0}")]
    Heartbeat(#[from] std::io::Error),
    #[error(transparent)]
    Verification(#[from] crate::store::errors::ControlError),
    #[error("run cancellation was requested before execution")]
    Cancelled,
}

impl From<crate::api::errors::ClientError> for WorkerError {
    fn from(error: crate::api::errors::ClientError) -> Self {
        match error {
            crate::api::errors::ClientError::Configuration(message) => Self::Configuration(message),
            crate::api::errors::ClientError::Transport(error) => Self::Transport(error),
            crate::api::errors::ClientError::Rejected(status) => Self::Rejected(status),
        }
    }
}
