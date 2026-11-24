//! The top-level entries of a project that hold given paths.

use std::path::{Component, Path, PathBuf};

/// The top-level entry of `root` holding each of `written`, relative paths read from `root`.
pub(crate) fn entries_holding(root: &Path, written: &[PathBuf]) -> Vec<String> {
    let real_root: PathBuf = resolved(root);
    written
        .iter()
        .filter_map(|path| top_entry(&real_root, &resolved(&root.join(path))))
        .collect()
}

/// The first component of `path` below `root`, if `path` lies inside it.
fn top_entry(root: &Path, path: &Path) -> Option<String> {
    if !path.starts_with(root) {
        return None;
    }
    match path.components().nth(root.components().count())? {
        Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
        _ => None,
    }
}

/// `path` with its deepest existing ancestor canonicalized and the rest appended, so a missing
/// path compares with its root in one form (`\\?\` on Windows, `/private` on macOS).
fn resolved(path: &Path) -> PathBuf {
    let mut existing: PathBuf = path.to_path_buf();
    let mut rest: Vec<Component<'_>> = Vec::new();
    let mut components = path.components();
    let base: PathBuf = loop {
        if let Ok(real) = std::fs::canonicalize(&existing) {
            break real;
        }
        match components.next_back() {
            Some(component) => {
                rest.push(component);
                existing = components.as_path().to_path_buf();
            }
            None => break PathBuf::new(),
        }
    };
    rest.into_iter().rev().fold(base, |mut joined, component| {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                joined.pop();
            }
            other => joined.push(other.as_os_str()),
        }
        joined
    })
}
