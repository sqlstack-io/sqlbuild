//! The changed files a delta compile lints, read as a full lint walk reads them.

use crate::rules::models::LintFile;
use pyo3::PyResult;
use sqlbuild_core::format_command::constants::LINT_DIRECTORY_NAMES;
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use std::path::{Path, PathBuf};

/// The changed files a delta compile lints, read as a full lint walk reads them.
pub fn changed_files(project_dir: &Path) -> PyResult<Vec<LintFile>> {
    let Some(injection) = injection() else {
        return Ok(Vec::new());
    };
    let mut files: Vec<LintFile> = Vec::new();
    for path in &injection.changed_paths {
        let linted: bool = path
            .split_once('/')
            .is_some_and(|(root, _)| LINT_DIRECTORY_NAMES.contains(&root));
        if !linted {
            continue;
        }
        let path: PathBuf = project_dir.join(path);
        let contents: String = std::fs::read_to_string(&path)
            .map_err(|error| pyo3::exceptions::PyOSError::new_err(error.to_string()))?;
        files.push(LintFile { path, contents });
    }
    Ok(files)
}
