//! The human-readable compile report.

use crate::compile_command::_helpers::invocation::report;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyString;

/// The human-readable compile report.
#[inline]
pub fn format_compile_text<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyString>> {
    report::format_compile_text(py, options)
}
