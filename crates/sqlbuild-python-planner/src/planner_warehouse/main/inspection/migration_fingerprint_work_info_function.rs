//! `migration_fingerprint_work_info` as a function of `module`.

use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::migration_fingerprint_work_info;

/// `migration_fingerprint_work_info` as a function of `module`.
pub fn migration_fingerprint_work_info_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(migration_fingerprint_work_info, module)
}
