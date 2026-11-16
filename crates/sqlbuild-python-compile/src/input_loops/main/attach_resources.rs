//! Pair every seed, then attach every function and source, in that order.

use crate::input_loops::_helpers::resources::attachment;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// Pair every seed, then attach every function and source, in that order.
#[inline]
pub fn attach_resources<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    attachment::attach_resources(py, options)
}
