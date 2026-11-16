//! `runtime_cursor_producer_names` as a function of `module`.

use crate::planner_strategy::main::cursor_inputs::runtime_cursor_producer_names::runtime_cursor_producer_names;

/// `runtime_cursor_producer_names` as a function of `module`.
pub fn runtime_cursor_producer_names_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(runtime_cursor_producer_names, module)
}
