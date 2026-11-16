//! `describe_full_rebuild_cause` as a function of `module`.

use crate::planner_phases::main::rebuilds::describe_full_rebuild_cause::describe_full_rebuild_cause;

/// `describe_full_rebuild_cause` as a function of `module`.
pub fn describe_full_rebuild_cause_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(describe_full_rebuild_cause, module)
}
