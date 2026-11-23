//! Every project entry outside the expected layout: the top level and the folders of `sql/` and
//! `python/`, for files of every type.

use std::fs;
use std::path::{Path, PathBuf};

use sqlbuild_core::constants::{
    PYTHON_CHILDREN, PYTHON_DIRECTORY, SQL_CHILDREN, SQL_DIRECTORY, TOP_LEVEL_DIRECTORIES,
    TOP_LEVEL_FILES,
};

use crate::project_layout::_helpers::ignored::IgnoreRules;
use crate::project_layout::constants::{
    DATABASE_COMPANION_SUFFIXES, DATABASE_FILE_SUFFIXES, HIDDEN_PREFIX, OUTPUT_DIRECTORIES,
    PYTHON_CACHE_DIRECTORY,
};
use crate::project_layout::models::LayoutProblem;

/// One folder the check reads and the entries it may hold.
struct Checked<'a> {
    folder: Option<&'static str>,
    directories: &'a [&'a str],
    files: &'a [&'a str],
}

/// Each entry of `project_dir` the layout does not allow, sorted by path.
///
/// `extra_paths` are project-relative entries the project keeps on purpose. Hidden entries,
/// `__pycache__`, SQLBuild's `target/` and `logs/`, local database files and gitignored entries
/// are always allowed.
pub fn layout_problems(project_dir: &Path, extra_paths: &[String]) -> Vec<LayoutProblem> {
    let root: PathBuf = fs::canonicalize(project_dir).unwrap_or_else(|_| project_dir.to_path_buf());
    let ignored: IgnoreRules = IgnoreRules::read(&root);
    let extra: Vec<String> = extra_paths
        .iter()
        .map(|path| path.replace('\\', "/").trim_end_matches('/').to_owned())
        .collect();
    let python_directories: Vec<&str> = PYTHON_CHILDREN
        .iter()
        .copied()
        .filter(|name| !name.ends_with(".py"))
        .collect();
    let python_files: Vec<&str> = PYTHON_CHILDREN
        .iter()
        .copied()
        .filter(|name| name.ends_with(".py"))
        .collect();
    let checked: [Checked<'_>; 3] = [
        Checked {
            folder: None,
            directories: &TOP_LEVEL_DIRECTORIES,
            files: &TOP_LEVEL_FILES,
        },
        Checked {
            folder: Some(SQL_DIRECTORY),
            directories: &SQL_CHILDREN,
            files: &[],
        },
        Checked {
            folder: Some(PYTHON_DIRECTORY),
            directories: &python_directories,
            files: &python_files,
        },
    ];
    let mut problems: Vec<LayoutProblem> = Vec::new();
    for folder in &checked {
        problems.extend(folder_problems(&root, folder, (&ignored, &extra)));
    }
    problems.sort_by(|left, right| left.path.cmp(&right.path));
    problems
}

fn folder_problems(
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

/// Entries every project may hold: hidden entries, bytecode caches, SQLBuild's own output folders
/// and local database files at the top level.
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
