//! `str(value)` of a Python string or string enum.

use crate::planner_strategy::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `str(value)` of a Python string or string enum.
#[inline]
pub fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    values::text(value)
}
