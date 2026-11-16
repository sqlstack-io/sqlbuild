//! The scope index fingerprint of `project_dir`, `None` when it cannot be read.

use crate::scopes::_helpers::index::scope_cache;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use std::path::PathBuf;

/// The scope index fingerprint of `project_dir`, `None` when it cannot be read.
#[inline]
pub fn project_fingerprint(py: Python<'_>, project_dir: PathBuf) -> PyResult<Option<String>> {
    scope_cache::project_fingerprint(py, project_dir)
}
