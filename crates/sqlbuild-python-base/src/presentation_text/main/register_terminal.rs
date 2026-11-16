//! Register the terminal functions and classes.

use crate::presentation_text::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the terminal functions and classes.
#[inline]
pub fn register_terminal(module: &Bound<'_, PyModule>) -> PyResult<()> {
    terminal::register(module)
}
