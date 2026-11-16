//! Every staged artifact's path relative to `staged_dir`.

use crate::compile_outputs::_helpers::artifacts::lossy;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_cache::artifacts::main::staged_files::staged_files;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use std::path::PathBuf;

/// Every staged artifact's path relative to `staged_dir`.
pub fn staged_artifact_files(py: Python<'_>, staged_dir: PathBuf) -> PyResult<Vec<String>> {
    compiler_guard(|| {
        let files: Vec<PathBuf> = py.detach(|| staged_files(&staged_dir));
        Ok(files.iter().map(|path| lossy(path)).collect())
    })
}
