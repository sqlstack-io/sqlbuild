//! Register the statements functions and classes.

use crate::microbatches::_helpers::statements;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the statements functions and classes.
#[inline]
pub fn register_statements(module: &Bound<'_, PyModule>) -> PyResult<()> {
    statements::register(module)
}
