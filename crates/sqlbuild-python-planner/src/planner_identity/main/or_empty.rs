//! `value or {}`.

use crate::planner_identity::_helpers::metadata;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `value or {}`.
#[inline]
pub fn or_empty<'py>(
    value: Option<Bound<'py, PyAny>>,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    metadata::or_empty(value, py)
}
