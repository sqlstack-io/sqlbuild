//! One field of a planner runtime passed as a Python object.

use crate::planner_phases::_helpers::inputs::runtime;
use crate::planner_phases::models::PlannerRuntime;
use pyo3::prelude::{Bound, Py, PyAny, PyResult};

/// One field of a planner runtime passed as a Python object.
#[inline]
pub fn runtime_field<'py>(
    runtime: &Bound<'py, PyAny>,
    field: fn(&PlannerRuntime) -> &Py<PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    runtime::runtime_field(runtime, field)
}
