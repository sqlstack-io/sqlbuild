//! The env file this process tree loaded, as messages show it.

use std::path::{Path, PathBuf};

use sqlbuild_core::process_environment::main::env_file_marker::env_file_marker;

/// `path` with symlinks and `..` resolved, through its parent when the file itself doesn't exist.
fn resolved(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => parent
            .canonicalize()
            .map_or_else(|_| path.to_path_buf(), |parent| parent.join(name)),
        _ => path.to_path_buf(),
    }
}

/// The env file's path relative to the project, such as `.env`, comparing resolved paths; `None` when the project loads none.
pub fn env_file_display(project_dir: &Path) -> Option<String> {
    let path = PathBuf::from(env_file_marker()?.path?);
    let file = resolved(&path);
    Some(match file.strip_prefix(resolved(project_dir)) {
        Ok(relative) => relative.to_string_lossy().into_owned(),
        Err(_outside_the_project) => file.to_string_lossy().into_owned(),
    })
}
