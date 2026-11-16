//! `None`, or the text.

use crate::planner_phases::main::text::text;
use pyo3::prelude::{Bound, PyAny, Python};

/// `None`, or the text.
pub fn optional_text<'py>(py: Python<'py>, value: Option<&str>) -> Bound<'py, PyAny> {
    value.map_or_else(|| py.None().into_bound(py), |value| text(py, value))
}
