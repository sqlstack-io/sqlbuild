//! `build_model_inputs` as a function of `module`.

use crate::compile_attachment::_helpers::model_inputs::build_model_inputs;

/// `build_model_inputs` as a function of `module`.
pub fn build_model_inputs_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_model_inputs, module)
}
