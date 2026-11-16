//! The project nodes of `field` by name.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The project nodes of `field` by name.
#[inline]
pub fn by_name<'py>(
    project: &Bound<'py, PyAny>,
    field: &str,
) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
    objects::by_name(project, field)
}
