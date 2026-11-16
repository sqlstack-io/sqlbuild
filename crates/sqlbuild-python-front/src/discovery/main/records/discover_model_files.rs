//! Discover the (selected) model files natively, reporting failing files to `on_fault`.

use crate::discovery::_helpers::records::model_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// Discover the (selected) model files natively, reporting failing files to `on_fault`.
#[inline]
pub fn discover_model_files<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    model_records::discover_model_files(py, options)
}
