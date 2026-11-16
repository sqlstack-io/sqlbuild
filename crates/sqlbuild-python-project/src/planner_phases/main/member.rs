//! A staying-Python enum member from its value.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_host::entries::models::HostId;

/// A staying-Python enum member from its value.
#[inline]
pub fn member<'py>(
    py: Python<'py>,
    class: impl Into<HostId>,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    objects::member(py, class, value)
}
