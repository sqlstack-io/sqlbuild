//! The Python exception a native config error raises as.

use crate::model_config::_helpers::model_nodes::model_config_builder;
use pyo3::PyErr;
use pyo3::prelude::Python;
use sqlbuild_model_config::errors::ConfigError;

/// The Python exception a native config error raises as.
#[inline]
pub fn raised_config_error(py: Python<'_>, error: ConfigError) -> PyErr {
    model_config_builder::raised_config_error(py, error)
}
