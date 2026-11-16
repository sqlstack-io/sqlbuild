//! A Python `str`.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::Python;

/// A Python `str`.
#[inline]
pub fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    objects::text(py, value)
}
