//! Register the records functions and classes.

use crate::compiled_views::_helpers::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the records functions and classes.
#[inline]
pub fn register_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    records::register(module)
}
