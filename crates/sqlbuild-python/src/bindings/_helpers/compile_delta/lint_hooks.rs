//! The SQL lint keys a full compile leaves for its image, and a delta compile's use of them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pyo3::PyResult;

use sqlbuild_core::format_command::constants::LINT_DIRECTORY_NAMES;
use sqlbuild_python_base::compile_context::main::delta::capture::capture;
use sqlbuild_python_base::compile_context::main::delta::capturing::capturing;
use sqlbuild_python_base::compile_context::main::delta::delta_unsupported::delta_unsupported;
use sqlbuild_python_base::compile_context::main::delta::injection::injection;

use crate::bindings::_helpers::rules::lint_pipeline::LintFile;

/// Each model file's lint key, recorded for the image or, in a delta, patched for reruns.
pub(crate) fn model_identities(fresh: Vec<(String, String)>) -> PyResult<Vec<(String, String)>> {
    let Some(injection) = injection() else {
        if capturing() {
            let keys: Vec<(String, String)> = fresh.clone();
            capture(|capture| capture.lint_model_keys = Some(keys));
        }
        return Ok(fresh);
    };
    let Some(stored) = injection.lint_model_keys.as_ref() else {
        return Err(delta_unsupported(
            "the lint keys of the models were not recorded",
        ));
    };
    let mut fresh: HashMap<String, String> = fresh.into_iter().collect();
    let whole: Vec<(String, String)> = stored
        .iter()
        .map(|(path, key)| match fresh.remove(path) {
            Some(key) => (path.clone(), key),
            None => (path.clone(), key.clone()),
        })
        .collect();
    if !fresh.is_empty() {
        return Err(delta_unsupported(
            "a recompiled model has no recorded lint key",
        ));
    }
    capture(|capture| capture.lint_model_keys = Some(whole.clone()));
    Ok(whole)
}

/// The changed files a delta compile lints, read as a full lint walk reads them.
pub(crate) fn changed_files(project_dir: &Path) -> PyResult<Vec<LintFile>> {
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

/// Each non-model file's lint key in walk order, recorded or, in a delta, patched.
pub(crate) fn file_keys(
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
