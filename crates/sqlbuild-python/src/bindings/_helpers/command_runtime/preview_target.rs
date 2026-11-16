//! Shared validation for inspection-only `--as` target previews.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_config::command_connection::main::check_preview_target::check_preview_target;

use crate::bindings::_helpers::command_runtime::runtime_errors::raised;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

fn target_names(config: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    native_field(config, "targets")?
        .try_iter()?
        .map(|name| name?.extract())
        .collect()
}

/// Fail clearly when `--as` names a target that is not configured.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, as_target, command_name))]
pub(crate) fn validate_preview_target(
    py: Python<'_>,
    discovered_inputs: Bound<'_, PyAny>,
    as_target: Option<String>,
    command_name: String,
) -> Result<(), PyErr> {
    let Some(as_target) = as_target else {
        return Ok(());
    };
    let mut configured: Vec<String> =
        target_names(&native_field(&discovered_inputs, "project_config")?)?;
    configured.extend(target_names(&native_field(
        &discovered_inputs,
        "local_config",
    )?)?);
    check_preview_target(&as_target, &command_name, &configured).map_err(|error| raised(py, error))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(validate_preview_target, module)?)
}
