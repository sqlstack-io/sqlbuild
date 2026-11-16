//! The Python hooks attached to one model, in lifecycle order without repeats.

use crate::graph::_helpers::nodes::execution_gates;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyTuple;

/// The Python hooks attached to one model, in lifecycle order without repeats.
#[inline]
pub fn model_python_hook_names<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    execution_gates::model_python_hook_names(py, model)
}
