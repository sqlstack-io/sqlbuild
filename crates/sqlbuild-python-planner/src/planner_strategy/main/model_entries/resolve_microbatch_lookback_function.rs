//! `resolve_microbatch_lookback` as a function of `module`.

use crate::planner_strategy::main::model_entries::resolve_microbatch_lookback::resolve_microbatch_lookback;

/// `resolve_microbatch_lookback` as a function of `module`.
pub fn resolve_microbatch_lookback_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(resolve_microbatch_lookback, module)
}
