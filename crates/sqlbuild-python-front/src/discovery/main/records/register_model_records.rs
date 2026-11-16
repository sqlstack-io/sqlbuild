//! Register the model records functions and classes.

use crate::discovery::_helpers::records::model_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the model records functions and classes.
#[inline]
pub fn register_model_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    model_records::register(module)
}
