//! Hold every registered record class, string enum and error class, read once while the module initialises.

use crate::records::_helpers::native_classes::construct;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Hold every registered record class, string enum and error class, read once while the module initialises.
#[inline]
pub fn capture(module: &Bound<'_, PyModule>) -> PyResult<()> {
    construct::capture(module)
}
