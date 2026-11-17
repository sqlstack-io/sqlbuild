//! The Python exception a native config error raises as.

use crate::model_config::_helpers::config::config_errors::config_exception;
use crate::model_config::errors::NativeConfigError;
use crate::model_config::main::config::native_config_error::native_config_error;
use pyo3::PyErr;
use pyo3::prelude::{Py, PyResult, Python};
use sqlbuild_model_config::errors::ConfigError;

/// The Python exception a native config error raises as.
pub fn raised_config_error(py: Python<'_>, error: ConfigError) -> PyErr {
    let raised = || -> PyResult<PyErr> {
        let error: Py<NativeConfigError> = native_config_error(py, error)?;
        Ok(PyErr::from_value(config_exception(
            error.bind(py).as_any(),
            None,
        )?))
    };
    raised().unwrap_or_else(|error| error)
}
