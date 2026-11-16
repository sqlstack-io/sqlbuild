//! Describe full rebuild cause.

use crate::planner_phases::_helpers::rebuilds::full_refresh;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Describe full rebuild cause.
#[inline]
pub fn describe_full_rebuild_cause(
    change_result: &Bound<'_, PyAny>,
    full_refresh: bool,
) -> PyResult<String> {
    full_refresh::describe_full_rebuild_cause(change_result, full_refresh)
}
