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
    /// The run request names a field this worker's release can't read, so it can't run all of what was asked.
    #[error(
        "this worker runs SQLBuild {version} and can't read a run request from a newer server (unknown field `{field}`); upgrade the location"
    )]
    NewerServer {
        version: &'static str,
        field: String,
    },
    #[error("worker API response could not be read: {0}")]
    Response(#[from] serde_json::Error),
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
