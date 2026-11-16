//! Register the versioned Python boundary for the native Rules engine.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;

#[pyfunction]
fn run_custom_host_json(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    py.compiler_detach(|| {
        sqlbuild_rules::engine::main::custom_host::run_custom_host_json(spec_json)
    })
    .map_err(value_error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_custom_host_json, module)?)?;
    crate::bindings::_helpers::rules::rows::register(module)
}
