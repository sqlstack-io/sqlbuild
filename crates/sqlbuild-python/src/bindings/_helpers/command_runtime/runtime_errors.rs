//! The Python exceptions command connection failures are raised as.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_config::command_connection::errors::CommandConnectionError;

use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

/// The `CliUserError` or `ProjectConfigError` for `error`.
pub(crate) fn raised(py: Python<'_>, error: CommandConnectionError) -> PyErr {
    let created = || -> PyResult<PyErr> {
        let instance: Bound<'_, PyAny> = match error {
            CommandConnectionError::User {
                code,
                message,
                help,
            } => {
                let keywords: Bound<'_, PyDict> = PyDict::new(py);
                keywords.set_item("code", code)?;
                keywords.set_item("help", help)?;
                error_class(py, ErrorId::CliUser)?.call((message,), Some(&keywords))?
            }
            CommandConnectionError::ProjectConfig(message) => {
                error_class(py, ErrorId::ProjectConfig)?.call1((message,))?
            }
        };
        Ok(PyErr::from_value(instance))
    };
    created().unwrap_or_else(|failure| failure)
}

/// Whether `error` is a `CompileInputError` or a `CliUserError`, the failures resolution reports.
pub(crate) fn expected_resolution_error(py: Python<'_>, error: &PyErr) -> PyResult<bool> {
    Ok(error.is_instance(py, &error_class(py, ErrorId::CliUser)?)
        || error.is_instance(py, &error_class(py, ErrorId::CompileInput)?))
}

/// The message a resolution error shows: `CliUserError.message`, else `str(error)`.
pub(crate) fn resolution_message(py: Python<'_>, error: &PyErr) -> PyResult<String> {
    let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
    if error.is_instance(py, &error_class(py, ErrorId::CliUser)?) {
        return native_field(&value, "message")?.str()?.extract();
    }
    value.str()?.extract()
}
