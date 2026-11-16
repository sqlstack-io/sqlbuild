//! Register the versioned Python boundary for the native Rules engine.

use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;

#[pyfunction]
pub(crate) fn run_custom_host_json(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    py.compiler_detach(|| {
        sqlbuild_rules::engine::main::custom_host::run_custom_host_json(spec_json)
    })
    .map_err(value_error)
}
