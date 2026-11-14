//! Why a declaration file cannot be read.

use crate::orchestration_files::models::FilePosition;

/// Why a declaration file cannot be read, at the position it points to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileError {
    pub message: String,
    pub position: FilePosition,
    pub help: Option<String>,
}
