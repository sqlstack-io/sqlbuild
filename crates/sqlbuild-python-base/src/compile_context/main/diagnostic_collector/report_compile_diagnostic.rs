//! Record a diagnostic in the active collection, or raise it when none is active.

use crate::compile_context::_helpers::diagnostic_collector;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Record a diagnostic in the active collection, or raise it when none is active.
#[inline]
pub fn report_compile_diagnostic(
    key: &Bound<'_, PyAny>,
    diagnostic: &Bound<'_, PyAny>,
) -> PyResult<()> {
    diagnostic_collector::report_compile_diagnostic(key, diagnostic)
}
