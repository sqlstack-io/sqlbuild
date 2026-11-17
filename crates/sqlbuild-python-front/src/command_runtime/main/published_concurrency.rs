//! The concurrency a published job without its own setting runs with on the compiled `target`, and its layer.

use crate::command_runtime::_helpers::concurrency;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_orchestration::declarations::models::ResolvedConcurrency;
use std::path::Path;

/// The concurrency a published job without its own setting runs with on the compiled `target`, and its layer.
#[inline]
pub fn published_concurrency(
    project_dir: &Path,
    discovered: &Bound<'_, PyAny>,
    target: Option<&str>,
) -> PyResult<ResolvedConcurrency> {
    concurrency::published_concurrency(project_dir, discovered, target)
}
