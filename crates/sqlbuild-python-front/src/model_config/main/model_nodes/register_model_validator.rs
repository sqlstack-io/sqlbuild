//! Register the model validator functions and classes.

use crate::model_config::models::NativeModelValidator;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the model validator functions and classes.
pub fn register_model_validator(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelValidator>()?;
    Ok(())
}
