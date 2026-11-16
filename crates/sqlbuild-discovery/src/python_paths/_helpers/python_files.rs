//! Every `.py` file below a project, as relative path parts, skipping ignored directories.

use std::fs;
use std::path::Path;

use crate::constants::PYTHON_FILE_SUFFIX;
use crate::python_paths::constants::{HIDDEN_PREFIX, IGNORED_ROOTS, PYTHON_CACHE_DIRECTORY};

/// The relative parts of every Python file in the `start` folder below `project_dir`, sorted part by part.
pub(crate) fn python_files(project_dir: &Path, start: &[&str]) -> Vec<Vec<String>> {
    let mut files: Vec<Vec<String>> = Vec::new();
    let mut pending: Vec<Vec<String>> = vec![start.iter().map(|part| (*part).to_owned()).collect()];
    while let Some(parts) = pending.pop() {
        let directory = parts
            .iter()
            .fold(project_dir.to_path_buf(), |path, part| path.join(part));
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let name: String = entry.file_name().to_string_lossy().into_owned();
            let followed_is_dir: bool = fs::metadata(entry.path()).is_ok_and(|meta| meta.is_dir());
            let mut child: Vec<String> = parts.clone();
            child.push(name.clone());
            if followed_is_dir {
                let is_link: bool = entry.file_type().is_ok_and(|kind| kind.is_symlink());
                if !is_link && walked(&parts, &name) {
                    pending.push(child);
                }
            } else if name.ends_with(PYTHON_FILE_SUFFIX) {
                files.push(child);
            }
        }
    }
    files.sort();
    files
}

/// Whether the walk enters directory `name` below `parts`.
fn walked(parts: &[String], name: &str) -> bool {
    !name.starts_with(HIDDEN_PREFIX)
        && name != PYTHON_CACHE_DIRECTORY
        && !(parts.is_empty() && IGNORED_ROOTS.contains(&name))
}
