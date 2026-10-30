//! Transport failures are distinct from rejected pinned execution inputs.

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("invalid worker connection: {0}")]
    Configuration(&'static str),
    #[error("worker API request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("worker API rejected the request with HTTP {0}")]
    Rejected(reqwest::StatusCode),
    #[error(transparent)]
    Verification(#[from] crate::store::errors::ControlError),
    #[error("run cancellation was requested before execution")]
    Cancelled,
}
