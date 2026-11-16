//! Shared error.

use crate::relation_naming::_helpers::fitting;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// Shared error.
#[inline]
pub fn shared_error(py: Python<'_>, message: String) -> PyErr {
    fitting::shared_error(py, message)
}
