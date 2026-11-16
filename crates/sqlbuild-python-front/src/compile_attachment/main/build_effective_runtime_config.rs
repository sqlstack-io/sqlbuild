//! Build environment, vars and invocation id without compiling resources.

use crate::compile_attachment::_helpers::effective_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Build environment, vars and invocation id without compiling resources.
#[inline]
pub fn build_effective_runtime_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    selected_target: Option<Bound<'py, PyAny>>,
    cli_vars: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    effective_config::build_effective_runtime_config(discovered_inputs, selected_target, cli_vars)
}
