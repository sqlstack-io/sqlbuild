//! Discover, compile and validate the project into one `CompileAnalysis`.

use crate::compile_driver::_helpers::stages::analysis::analyze;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;

/// Discover, compile and validate the project into one `CompileAnalysis`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn analyze_compile_project<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    analyze(py, &options.unwrap_or_else(|| PyDict::new(py)))
}
