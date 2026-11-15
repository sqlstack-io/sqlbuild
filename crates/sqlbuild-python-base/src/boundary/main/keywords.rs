//! Keyword arguments built from `(name, value)` pairs, in order.

use crate::boundary::_helpers::python_calls;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Keyword arguments built from `(name, value)` pairs, in order.
#[inline]
pub fn keywords<'py>(
    py: Python<'py>,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyDict>> {
    python_calls::keywords(py, pairs)
}
