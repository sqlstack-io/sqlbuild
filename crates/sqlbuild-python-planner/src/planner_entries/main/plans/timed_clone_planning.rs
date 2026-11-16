//! Plan a clone, with the seconds spent inspecting sources and planning.

use crate::planner_entries::_helpers::plans::clone_plans;
use crate::planner_entries::types::PhaseTimings;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};

/// Plan a clone, with the seconds spent inspecting sources and planning.
#[inline]
pub fn timed_clone_planning<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<(Bound<'py, PyTuple>, PhaseTimings)> {
    clone_plans::timed_clone_planning(py, kwargs)
}
