//! Every project entry outside the layout, at the top level and inside `sql/` and `python/`.

use std::fs;
use std::path::{Path, PathBuf};

use sqlbuild_core::constants::{
    PYTHON_CHILDREN, PYTHON_DIRECTORY, SQL_CHILDREN, SQL_DIRECTORY, TOP_LEVEL_DIRECTORIES,
    TOP_LEVEL_FILES,
};

use crate::project_layout::_helpers::folders::folder_problems;
use crate::project_layout::_helpers::ignored::IgnoreRules;
use crate::project_layout::models::{Checked, LayoutProblem};

/// Each entry of `project_dir` the layout does not allow, other than `extra_paths`, sorted by path.
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
