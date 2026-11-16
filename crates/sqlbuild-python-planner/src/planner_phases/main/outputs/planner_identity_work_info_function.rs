//! `planner_identity_work_info` as a function of `module`.

use crate::planner_phases::_helpers::outputs::identities::planner_identity_work_info;

/// `planner_identity_work_info` as a function of `module`.
pub fn planner_identity_work_info_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(planner_identity_work_info, module)
}
