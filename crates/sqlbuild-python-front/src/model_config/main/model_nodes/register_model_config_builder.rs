//! Register the model config builder functions and classes.

use crate::model_config::models::NativeModelConfigBuilder;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the model config builder functions and classes.
pub fn register_model_config_builder(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelConfigBuilder>()?;
    Ok(())
}
