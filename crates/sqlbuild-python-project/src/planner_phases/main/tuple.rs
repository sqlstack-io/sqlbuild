//! A tuple of the items.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;

/// A tuple of the items.
pub fn tuple<'py>(py: Python<'py>, items: Vec<Bound<'py, PyAny>>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, items)?.into_any())
}
