//! The strings of any Python iterable, in iteration order.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The strings of any Python iterable, in iteration order.
#[inline]
pub fn strings(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    objects::strings(values)
}
