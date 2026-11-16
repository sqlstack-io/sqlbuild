//! A dictionary of the entries, in order.

use crate::plan_output::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// A dictionary of the entries, in order.
#[inline]
pub fn dict<'py>(
    py: Python<'py>,
    entries: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyDict>> {
    values::dict(py, entries)
}
