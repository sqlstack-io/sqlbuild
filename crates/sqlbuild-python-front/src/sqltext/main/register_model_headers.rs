//! Register the model headers functions and classes.

use crate::sqltext::_helpers::model_headers;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the model headers functions and classes.
#[inline]
pub fn register_model_headers(module: &Bound<'_, PyModule>) -> PyResult<()> {
    model_headers::register(module)
}
