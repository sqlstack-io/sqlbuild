//! `native_model_config_session` as a function of `module`.

use crate::compile_attachment::_helpers::model_config::native_model_config_session;

/// `native_model_config_session` as a function of `module`.
pub fn native_model_config_session_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(native_model_config_session, module)
}
