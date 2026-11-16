//! `value or {}`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;

/// `value or {}`.
pub fn or_empty<'py>(
    value: Option<Bound<'py, PyAny>>,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Some(value) if value.is_truthy()? => Ok(value),
        _ => Ok(PyDict::new(py).into_any()),
    }
}
