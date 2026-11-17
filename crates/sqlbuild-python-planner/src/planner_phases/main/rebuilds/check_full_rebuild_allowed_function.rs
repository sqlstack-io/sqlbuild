//! `check_full_rebuild_allowed` as a function of `module`.

use crate::planner_phases::main::rebuilds::check_full_rebuild_allowed::check_full_rebuild_allowed;

/// `check_full_rebuild_allowed` as a function of `module`.
pub fn check_full_rebuild_allowed_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(check_full_rebuild_allowed, module)
}
