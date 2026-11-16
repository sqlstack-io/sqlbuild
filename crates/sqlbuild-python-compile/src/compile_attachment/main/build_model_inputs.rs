//! Attach schema metadata to discovered model files.

use crate::compile_attachment::_helpers::model_inputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// Attach schema metadata to discovered model files.
#[inline]
pub fn build_model_inputs<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    model_inputs::build_model_inputs(py, options)
}
