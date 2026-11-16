//! `apply_microbatch_limit` as a function of `module`.

use crate::planner_strategy::_helpers::microbatch_limits::apply_microbatch_limit;

/// `apply_microbatch_limit` as a function of `module`.
pub fn apply_microbatch_limit_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(apply_microbatch_limit, module)
}
