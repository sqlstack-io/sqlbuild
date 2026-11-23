//! The top-level entries of a project that hold given paths.

use std::path::{Component, Path, PathBuf};

/// The top-level entry of `root` holding each of `written`, relative paths read from `root`.
pub(crate) fn entries_holding(root: &Path, written: &[PathBuf]) -> Vec<String> {
    let real_root: PathBuf = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    written
        .iter()
        .filter_map(|path| top_entry(&real_root, &root.join(path)))
        .collect()
}

/// The first component of `path` below `root`, if `path` lies inside it.
fn top_entry(root: &Path, path: &Path) -> Option<String> {
    let real: PathBuf = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !real.starts_with(root) {
        return None;
    }
    match real.components().nth(root.components().count())? {
        Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
        _ => None,
    }
}
