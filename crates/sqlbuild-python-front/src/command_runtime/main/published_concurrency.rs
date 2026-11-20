//! The concurrency a published job without its own setting runs with on the compiled `target`.

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_orchestration::declarations::models::ResolvedConcurrency;

use crate::command_runtime::main::resolve_concurrency::resolve_concurrency;

/// The concurrency a published job without its own setting runs with on the compiled `target`.
pub fn published_concurrency(
    discovered: &Bound<'_, PyAny>,
    target: Option<&str>,
) -> PyResult<ResolvedConcurrency> {
    let py = discovered.py();
    let effective = resolve_concurrency(
        &py.None().into_bound(py),
        discovered,
        target.into_pyobject(py)?.as_any(),
    )?;
    let limit = u64::try_from(effective.limit)
        .map_err(|_| PyValueError::new_err("concurrency must be at least 1"))?;
    Ok(ResolvedConcurrency { limit })
}
