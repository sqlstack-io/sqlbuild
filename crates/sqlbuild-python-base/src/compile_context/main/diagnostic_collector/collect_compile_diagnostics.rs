//! A new collection; entering it collects diagnostics instead of raising the first one.

use crate::compile_context::_helpers::diagnostic_collector;
use crate::compile_context::models::CollectedCompileDiagnostics;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// A new collection; entering it collects diagnostics instead of raising the first one.
#[inline]
pub fn collect_compile_diagnostics(
    py: Python<'_>,
) -> PyResult<Bound<'_, CollectedCompileDiagnostics>> {
    diagnostic_collector::collect_compile_diagnostics(py)
}
