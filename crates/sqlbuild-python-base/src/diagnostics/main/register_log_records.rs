//! Register the log records functions and classes.

use crate::diagnostics::_helpers::log_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the log records functions and classes.
#[inline]
pub fn register_log_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    log_records::register(module)
}
