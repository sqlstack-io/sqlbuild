//! Delete `target/compiled-failed/`, which only the latest compile whose Rules failed owns.

use pyo3::prelude::PyResult;
use sqlbuild_cache::project_reuse::constants::FAILED_COMPILED_DIRECTORY_NAME;
use std::path::Path;

/// Delete `target/compiled-failed/`, which only the latest compile whose Rules failed owns.
pub fn remove_failed_compile(target: &Path) -> PyResult<()> {
    match std::fs::remove_dir_all(target.join(FAILED_COMPILED_DIRECTORY_NAME)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}
