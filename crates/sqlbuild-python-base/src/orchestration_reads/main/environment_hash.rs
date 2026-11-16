//! A lockfile binds declared dependencies; without one, bind installed distribution versions.

use crate::orchestration_reads::_helpers::orchestration_environment;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use std::path::Path;

/// A lockfile binds declared dependencies; without one, bind installed distribution versions.
#[inline]
pub fn environment_hash(
    py: Python<'_>,
    project_dir: &Path,
    sqlbuild_version: &str,
) -> PyResult<String> {
    orchestration_environment::environment_hash(py, project_dir, sqlbuild_version)
}
