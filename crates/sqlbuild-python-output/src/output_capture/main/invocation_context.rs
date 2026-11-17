//! Validated opaque invocation metadata supplied by an integration through the environment.

use crate::output_capture::_helpers::records::input_error;
use crate::output_capture::_helpers::records::validate_depth;
use crate::output_capture::_helpers::values::freeze;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_core::command_output::constants::{
    INVOCATION_CONTEXT_ENV, MAX_INVOCATION_CONTEXT_BYTES,
};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

/// Validated opaque invocation metadata supplied by an integration through the environment.
pub fn invocation_context<'py>(
    py: Python<'py>,
    environment: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let environment = match environment {
        Some(environment) if !environment.is_none() => environment,
        _ => imported(py, "os", "environ")?,
    };
    let raw = mapping_get(&environment, INVOCATION_CONTEXT_ENV)?;
    if raw.is_none() {
        return Ok(PyDict::new(py).into_any());
    }
    let text: String = raw.extract()?;
    if text.len() > MAX_INVOCATION_CONTEXT_BYTES {
        return Err(input_error(
            py,
            &format!(
                "{INVOCATION_CONTEXT_ENV} must not exceed {MAX_INVOCATION_CONTEXT_BYTES} bytes"
            ),
        ));
    }
    let loaded = imported(py, "json", "loads")?
        .call1((text,))
        .map_err(|error| match error.is_instance_of::<PyValueError>(py) {
            true => input_error(
                py,
                &format!("{INVOCATION_CONTEXT_ENV} must contain valid JSON"),
            ),
            false => error,
        })?;
    if !loaded.is_instance_of::<PyDict>() {
        return Err(input_error(
            py,
            &format!("{INVOCATION_CONTEXT_ENV} must contain a JSON object"),
        ));
    }
    validate_depth(&loaded, 1)?;
    freeze(&loaded, "external_context").map_err(|error| {
        match error.is_instance_of::<PyValueError>(py) {
            true => input_error(py, &error.value(py).to_string()),
            false => error,
        }
    })
}
