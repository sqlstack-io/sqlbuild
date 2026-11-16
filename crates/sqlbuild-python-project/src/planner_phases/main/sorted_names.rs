//! `sorted(items)` as a tuple of strings.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `sorted(items)` as a tuple of strings.
#[inline]
pub fn sorted_names<'py>(py: Python<'py>, names: Vec<String>) -> PyResult<Bound<'py, PyAny>> {
    objects::sorted_names(py, names)
}
