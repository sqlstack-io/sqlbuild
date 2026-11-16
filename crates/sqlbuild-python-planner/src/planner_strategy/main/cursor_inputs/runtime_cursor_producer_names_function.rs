//! `runtime_cursor_producer_names` as a function of `module`.

use crate::planner_strategy::_helpers::cursor_inputs;

/// `runtime_cursor_producer_names` as a function of `module`.
#[inline]
pub fn runtime_cursor_producer_names_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    cursor_inputs::runtime_cursor_producer_names_function(module)
}
