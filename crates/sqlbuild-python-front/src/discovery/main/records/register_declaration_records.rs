//! Register the declaration records functions and classes.

use crate::discovery::_helpers::records::declaration_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the declaration records functions and classes.
#[inline]
pub fn register_declaration_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    declaration_records::register(module)
}
