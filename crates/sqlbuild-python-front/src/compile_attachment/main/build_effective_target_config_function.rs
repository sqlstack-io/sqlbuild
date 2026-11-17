//! `build_effective_target_config` as a function of `module`.

use crate::compile_attachment::main::build_effective_target_config::build_effective_target_config;

/// `build_effective_target_config` as a function of `module`.
pub fn build_effective_target_config_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    pyo3::wrap_pyfunction!(build_effective_target_config, module)
}
