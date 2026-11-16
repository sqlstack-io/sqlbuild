//! Each model file's lint key, recorded for the image or, in a delta, patched for reruns.

use pyo3::PyResult;
use sqlbuild_python_base::compile_context::main::delta::capture::capture;
use sqlbuild_python_base::compile_context::main::delta::capturing::capturing;
use sqlbuild_python_base::compile_context::main::delta::delta_unsupported::delta_unsupported;
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use std::collections::HashMap;

/// Each model file's lint key, recorded for the image or, in a delta, patched for reruns.
pub fn model_identities(fresh: Vec<(String, String)>) -> PyResult<Vec<(String, String)>> {
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
