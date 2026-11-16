//! `config_exception` as the raised error, or the failure building it.

use crate::model_config::_helpers::config::config_errors;
use pyo3::PyErr;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;

/// `config_exception` as the raised error, or the failure building it.
#[inline]
pub fn raised_config_exception(
    error: &Bound<'_, PyAny>,
    values: Option<&Bound<'_, PyAny>>,
) -> PyErr {
    config_errors::raised_config_exception(error, values)
}
