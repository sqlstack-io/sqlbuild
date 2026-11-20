//! The env file this process tree loaded, as messages show it.

use std::path::Path;

use sqlbuild_core::process_environment::main::env_file_marker::env_file_marker;
use sqlbuild_core::process_environment::main::project_relative_path::project_relative_path;

/// The env file's path relative to the project, such as `.env`; `None` when the project loads none.
pub fn env_file_display(project_dir: &Path) -> Option<String> {
    let path = env_file_marker()?.path?;
    Some(project_relative_path(project_dir, Path::new(&path)))
}
