//! A persisted cost run that cannot be read.

use std::fmt;

/// The reason a cost run artifact is unusable, as `CostArtifactError` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostArtifactError {
    pub message: String,
}

impl CostArtifactError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CostArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
