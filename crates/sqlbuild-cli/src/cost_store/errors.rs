//! Cost artifact failures, distinguished from filesystem failures at the Python edge.

use sqlbuild_core::cost_report::errors::CostArtifactError;
use std::io;

#[derive(Debug)]
pub enum CostStoreError {
    Io(io::Error),
    Artifact(CostArtifactError),
}

impl From<io::Error> for CostStoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<CostArtifactError> for CostStoreError {
    fn from(error: CostArtifactError) -> Self {
        Self::Artifact(error)
    }
}
