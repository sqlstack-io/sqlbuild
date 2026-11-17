//! `detect_stale_warning_changes_of` as a function of `module`.

use crate::planner_phases::_helpers::outputs::identities::detect_stale_warning_changes_of;

/// `detect_stale_warning_changes_of` as a function of `module`.
pub fn detect_stale_warning_changes_of_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(detect_stale_warning_changes_of, module)
}
