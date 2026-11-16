//! The staying-Python class `class(**fields)`.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_host::entries::models::HostId;

/// The staying-Python class `class(**fields)`.
#[inline]
pub fn construct<'py>(
    py: Python<'py>,
    class: impl Into<HostId>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    objects::construct(py, class, fields)
}
