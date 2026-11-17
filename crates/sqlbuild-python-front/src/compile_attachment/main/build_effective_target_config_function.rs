//! `build_effective_target_config` as a function of `module`.

use crate::compile_attachment::_helpers::effective_config;

/// `build_effective_target_config` as a function of `module`.
#[inline]
pub fn build_effective_target_config_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    effective_config::build_effective_target_config_function(module)
}
