//! Register the console records functions and classes.

use crate::diagnostics::_helpers::console_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the console records functions and classes.
#[inline]
pub fn register_console_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    console_records::register(module)
}
