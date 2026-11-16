//! `cursor_input_columns_of` as a function of `module`.

use crate::planner_strategy::_helpers::cursor_inputs::cursor_input_columns_of;

/// `cursor_input_columns_of` as a function of `module`.
pub fn cursor_input_columns_of_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(cursor_input_columns_of, module)
}
