//! `PlannerInputError(message, code=code, help=help)`.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// `PlannerInputError(message, code=code, help=help)`.
#[inline]
pub fn planner_error(
    py: Python<'_>,
    message: &str,
    (code, help): (Option<&str>, Option<&str>),
) -> PyErr {
    objects::planner_error(py, message, (code, help))
}
