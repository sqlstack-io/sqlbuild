//! Register the style functions and classes.

use crate::presentation_text::_helpers::style;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the style functions and classes.
#[inline]
pub fn register_style(module: &Bound<'_, PyModule>) -> PyResult<()> {
    style::register(module)
}
