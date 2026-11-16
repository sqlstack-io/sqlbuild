//! Register the header declarations functions and classes.

use crate::discovery::_helpers::headers::header_declarations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the header declarations functions and classes.
#[inline]
pub fn register_header_declarations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    header_declarations::register(module)
}
