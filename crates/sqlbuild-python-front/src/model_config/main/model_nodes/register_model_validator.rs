//! Register the model validator functions and classes.

use crate::model_config::_helpers::model_nodes::model_validator;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the model validator functions and classes.
#[inline]
pub fn register_model_validator(module: &Bound<'_, PyModule>) -> PyResult<()> {
    model_validator::register(module)
}
