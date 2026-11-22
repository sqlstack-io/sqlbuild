//! A path as messages show it: relative to the project when it lies inside it.

use std::path::{Path, PathBuf};

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

/// `path` relative to `project_dir`, comparing resolved paths so relative and symlinked spellings agree; else resolved in full.
#[must_use]
pub fn project_relative_path(project_dir: &Path, path: &Path) -> String {
    let file = resolved(path);
    match file.strip_prefix(resolved(project_dir)) {
        Ok(relative) => relative.to_string_lossy().into_owned(),
        Err(_outside_the_project) => file.to_string_lossy().into_owned(),
    }
}
