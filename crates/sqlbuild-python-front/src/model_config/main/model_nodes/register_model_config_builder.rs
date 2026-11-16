//! Register the model config builder functions and classes.

use crate::model_config::_helpers::model_nodes::model_config_builder;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the model config builder functions and classes.
#[inline]
pub fn register_model_config_builder(module: &Bound<'_, PyModule>) -> PyResult<()> {
    model_config_builder::register(module)
}
