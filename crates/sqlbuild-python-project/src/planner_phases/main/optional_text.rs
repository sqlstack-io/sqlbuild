//! `None`, or the text.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::Python;

/// `None`, or the text.
#[inline]
pub fn optional_text<'py>(py: Python<'py>, value: Option<&str>) -> Bound<'py, PyAny> {
    objects::optional_text(py, value)
}
