//! A frozenset of the items.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// A frozenset of the items.
#[inline]
pub fn frozenset<'py>(py: Python<'py>, items: &[Bound<'py, PyAny>]) -> PyResult<Bound<'py, PyAny>> {
    objects::frozenset(py, items)
}
