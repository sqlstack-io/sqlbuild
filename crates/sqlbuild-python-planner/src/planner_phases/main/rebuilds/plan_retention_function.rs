//! `plan_retention` as a function of `module`.

use crate::planner_phases::_helpers::rebuilds::retention;

/// `plan_retention` as a function of `module`.
#[inline]
pub fn plan_retention_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    retention::plan_retention_function(module)
}
