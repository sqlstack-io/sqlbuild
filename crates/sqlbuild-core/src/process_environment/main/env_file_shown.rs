//! The env file this process tree loaded, as messages name it.

use std::path::Path;

use crate::process_environment::main::env_file_marker::env_file_marker;
use crate::process_environment::main::project_relative_path::project_relative_path;

/// The env file's path from the project it was chosen for, such as `.env`; `None` when none is loaded.
#[must_use]
pub fn env_file_shown() -> Option<String> {
    let marker = env_file_marker()?;
    let path: String = marker.path?;
    Some(project_relative_path(
        Path::new(&marker.project_dir),
        Path::new(&path),
    ))
}
