//! `plan_audit_of` as a function of `module`.

use crate::planner_strategy::_helpers::audit_entries;

/// `plan_audit_of` as a function of `module`.
#[inline]
pub fn plan_audit_of_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    audit_entries::plan_audit_of_function(module)
}
