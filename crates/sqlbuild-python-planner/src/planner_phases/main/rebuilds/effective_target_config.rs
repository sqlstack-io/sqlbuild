//! The resolved config of the planned target, or defaults without one.

use crate::planner_phases::_helpers::rebuilds::retention;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The resolved config of the planned target, or defaults without one.
#[inline]
pub fn effective_target_config<'py>(runtime: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    retention::effective_target_config(runtime)
}
