//! Register the header columns functions and classes.

use crate::model_config::_helpers::headers::header_columns;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the header columns functions and classes.
#[inline]
pub fn register_header_columns(module: &Bound<'_, PyModule>) -> PyResult<()> {
    header_columns::register(module)
}
