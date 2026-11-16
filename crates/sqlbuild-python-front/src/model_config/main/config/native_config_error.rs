//! Wrap `error` for Python.

use crate::model_config::_helpers::config::config_errors;
use crate::model_config::errors::NativeConfigError;
use pyo3::Py;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_model_config::errors::ConfigError;

/// Wrap `error` for Python.
#[inline]
pub fn native_config_error(py: Python<'_>, error: ConfigError) -> PyResult<Py<NativeConfigError>> {
    config_errors::native_config_error(py, error)
}
