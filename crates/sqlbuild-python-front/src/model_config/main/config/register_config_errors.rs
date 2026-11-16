//! Register the config errors functions and classes.

use crate::model_config::_helpers::config::config_errors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the config errors functions and classes.
#[inline]
pub fn register_config_errors(module: &Bound<'_, PyModule>) -> PyResult<()> {
    config_errors::register(module)
}
