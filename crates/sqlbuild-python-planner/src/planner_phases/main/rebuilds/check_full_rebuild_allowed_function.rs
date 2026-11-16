//! `check_full_rebuild_allowed` as a function of `module`.

use crate::planner_phases::_helpers::rebuilds::full_refresh;

/// `check_full_rebuild_allowed` as a function of `module`.
#[inline]
pub fn check_full_rebuild_allowed_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    full_refresh::check_full_rebuild_allowed_function(module)
}
