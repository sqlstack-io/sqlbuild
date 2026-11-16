//! `cursor_input_relations_of` as a function of `module`.

use crate::planner_strategy::_helpers::cursor_inputs;

/// `cursor_input_relations_of` as a function of `module`.
#[inline]
pub fn cursor_input_relations_of_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    cursor_inputs::cursor_input_relations_of_function(module)
}
