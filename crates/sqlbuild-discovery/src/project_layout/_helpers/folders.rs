//! The entries of one folder that the project layout does not allow.

use std::fs;
use std::path::{Path, PathBuf};

use crate::project_layout::_helpers::ignored::IgnoreRules;
use crate::project_layout::constants::{
    DATABASE_COMPANION_SUFFIXES, DATABASE_FILE_SUFFIXES, HIDDEN_PREFIX, OUTPUT_DIRECTORIES,
    PYTHON_CACHE_DIRECTORY,
};
use crate::project_layout::models::{Checked, LayoutProblem};

/// The entries of one checked folder that the layout does not allow.
pub(crate) fn folder_problems(
    root: &Path,
    checked: &Checked<'_>,
    (ignored, extra): (&IgnoreRules, &[String]),
) -> Vec<LayoutProblem> {
    let directory: PathBuf = checked
        .folder
        .map_or(root.to_path_buf(), |name| root.join(name));
    let Ok(entries) = fs::read_dir(&directory) else {
        return Vec::new();
    };
    let mut problems: Vec<LayoutProblem> = Vec::new();
    for entry in entries.flatten() {
        let name: String = entry.file_name().to_string_lossy().into_owned();
        let is_dir: bool = fs::metadata(entry.path()).is_ok_and(|meta| meta.is_dir());
        let relative: String = checked
            .folder
            .map_or(name.clone(), |folder| format!("{folder}/{name}"));
        let expected: bool = if is_dir {
            checked.directories.contains(&name.as_str())
        } else {
            checked.files.contains(&name.as_str())
        };
        if expected
            || always_allowed(checked.folder, &name, is_dir)
            || extra.contains(&relative)
            || ignored.is_ignored(&entry.path(), is_dir)
        {
            continue;
        }
        problems.push(LayoutProblem {
            path: if is_dir {
                format!("{relative}/")
            } else {
                relative
            },
            folder: checked.folder,
            allowed: allowed(checked),
        });
    }
    problems
}

/// Hidden entries, bytecode caches, and top-level output folders and local database files.
fn always_allowed(folder: Option<&str>, name: &str, is_dir: bool) -> bool {
    if name.starts_with(HIDDEN_PREFIX) || name == PYTHON_CACHE_DIRECTORY {
        return true;
    }
    if folder.is_some() {
        return false;
    }
    if is_dir {
        OUTPUT_DIRECTORIES.contains(&name)
    } else {
        is_database_file(name)
    }
}

/// A local database file, or the log, shared-memory or journal file beside one.
fn is_database_file(name: &str) -> bool {
    let base: &str = DATABASE_COMPANION_SUFFIXES
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
        .unwrap_or(name);
    DATABASE_FILE_SUFFIXES
        .iter()
        .any(|suffix| base.ends_with(suffix))
}

/// The folders `checked` may hold, as the help lists them.
fn allowed(checked: &Checked<'_>) -> Vec<String> {
    checked
        .directories
        .iter()
        .map(|name| match checked.folder {
            Some(folder) => format!("{folder}/{name}/"),
            None => format!("{name}/"),
        })
        .collect()
}
