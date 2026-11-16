//! Register the declaration files functions and classes.

use crate::discovery::_helpers::sources::declaration_files;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the declaration files functions and classes.
#[inline]
pub fn register_declaration_files(module: &Bound<'_, PyModule>) -> PyResult<()> {
    declaration_files::register(module)
}
