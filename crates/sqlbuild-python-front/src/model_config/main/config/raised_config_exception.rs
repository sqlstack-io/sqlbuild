//! `config_exception` as the raised error, or the failure building it.

use crate::model_config::_helpers::config::config_errors::config_exception;
use pyo3::PyErr;
use pyo3::prelude::{Bound, PyAny};

/// `config_exception` as the raised error, or the failure building it.
pub fn raised_config_exception(
    error: &Bound<'_, PyAny>,
    values: Option<&Bound<'_, PyAny>>,
) -> PyErr {
    match config_exception(error, values) {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}
