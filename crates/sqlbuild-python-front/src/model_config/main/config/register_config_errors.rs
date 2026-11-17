//! Register the config errors functions and classes.

use crate::model_config::errors::NativeConfigError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the config errors functions and classes.
pub fn register_config_errors(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeConfigError>()?;
    Ok(())
}
