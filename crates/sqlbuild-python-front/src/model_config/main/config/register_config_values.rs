//! Register the config values functions and classes.

use crate::model_config::_helpers::config::config_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the config values functions and classes.
#[inline]
pub fn register_config_values(module: &Bound<'_, PyModule>) -> PyResult<()> {
    config_values::register(module)
}
