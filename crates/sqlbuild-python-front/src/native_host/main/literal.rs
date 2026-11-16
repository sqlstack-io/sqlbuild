//! Literal.

use crate::native_host::_helpers::consumption_state;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Literal.
#[inline]
pub fn literal(py: Python<'_>, value: Option<&str>) -> PyResult<String> {
    consumption_state::literal(py, value)
}
