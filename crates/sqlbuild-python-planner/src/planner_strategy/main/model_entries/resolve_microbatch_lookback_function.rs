//! `resolve_microbatch_lookback` as a function of `module`.

use crate::planner_strategy::_helpers::model_entries;

/// `resolve_microbatch_lookback` as a function of `module`.
#[inline]
pub fn resolve_microbatch_lookback_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    model_entries::resolve_microbatch_lookback_function(module)
}
