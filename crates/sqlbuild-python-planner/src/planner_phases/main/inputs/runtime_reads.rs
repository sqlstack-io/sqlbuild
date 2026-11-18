//! The warehouse reads of a planner runtime passed as a Python object.

use crate::planner_phases::_helpers::inputs::runtime;
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// The warehouse reads of a planner runtime passed as a Python object.
#[inline]
pub fn runtime_reads<'py>(runtime: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PlannerReads>> {
    runtime::runtime_reads(runtime)
}
