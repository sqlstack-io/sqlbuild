//! Register the document functions and classes.

use crate::presentation_text::_helpers::document;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the document functions and classes.
#[inline]
pub fn register_document(module: &Bound<'_, PyModule>) -> PyResult<()> {
    document::register(module)
}
