//! Each non-model file's lint key in walk order, recorded or, in a delta, patched.

use pyo3::PyResult;
use sqlbuild_python_base::compile_context::main::delta::capture::capture;
use sqlbuild_python_base::compile_context::main::delta::capturing::capturing;
use sqlbuild_python_base::compile_context::main::delta::delta_unsupported::delta_unsupported;
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Each non-model file's lint key in walk order, recorded or, in a delta, patched.
pub fn file_keys(
    project_dir: &Path,
    fresh: Vec<(PathBuf, String, Option<String>)>,
) -> PyResult<Vec<(PathBuf, String, Option<String>)>> {
    let Some(injection) = injection() else {
        if capturing() {
            let keys: Vec<(String, Option<String>)> = fresh
                .iter()
                .map(|(_, path, key)| (path.clone(), key.clone()))
                .collect();
            capture(|capture| capture.lint_file_keys = Some(keys));
        }
        return Ok(fresh);
    };
    let Some(stored) = injection.lint_file_keys.as_ref() else {
        return Err(delta_unsupported(
            "the lint keys of the project files were not recorded",
        ));
    };
    let mut fresh: HashMap<String, (PathBuf, String, Option<String>)> =
        fresh.into_iter().map(|row| (row.1.clone(), row)).collect();
    let whole: Vec<(PathBuf, String, Option<String>)> = stored
        .iter()
        .map(|(path, key)| {
            fresh
                .remove(path)
                .unwrap_or_else(|| (project_dir.join(path), path.clone(), key.clone()))
        })
        .collect();
    if !fresh.is_empty() {
        return Err(delta_unsupported("a changed file has no recorded lint key"));
    }
    let keys: Vec<(String, Option<String>)> = whole
        .iter()
        .map(|(_, path, key)| (path.clone(), key.clone()))
        .collect();
    capture(|capture| capture.lint_file_keys = Some(keys));
    Ok(whole)
}
