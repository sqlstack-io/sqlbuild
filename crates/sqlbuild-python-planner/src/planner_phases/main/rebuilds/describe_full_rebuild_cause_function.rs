//! `describe_full_rebuild_cause` as a function of `module`.

use crate::planner_phases::_helpers::rebuilds::full_refresh;

/// `describe_full_rebuild_cause` as a function of `module`.
#[inline]
pub fn describe_full_rebuild_cause_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    full_refresh::describe_full_rebuild_cause_function(module)
}
