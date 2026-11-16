//! `report_compile_diagnostic` as a Python function, for the callbacks user macros receive.

use crate::compile_context::_helpers::diagnostic_collector;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyCFunction;

/// `report_compile_diagnostic` as a Python function, for the callbacks user macros receive.
#[inline]
pub fn report_compile_diagnostic_function(py: Python<'_>) -> PyResult<Bound<'_, PyCFunction>> {
    diagnostic_collector::report_compile_diagnostic_function(py)
}
