//! `apply_configured_microbatch_limits` as a function of `module`.

use crate::planner_strategy::main::microbatch_limits::apply_configured_microbatch_limits::apply_configured_microbatch_limits;

/// `apply_configured_microbatch_limits` as a function of `module`.
pub fn apply_configured_microbatch_limits_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(apply_configured_microbatch_limits, module)
}
