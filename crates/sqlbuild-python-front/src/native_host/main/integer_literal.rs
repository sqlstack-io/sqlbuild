//! Integer literal.

use crate::native_host::_helpers::consumption_state;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Integer literal.
#[inline]
pub fn integer_literal(py: Python<'_>, value: Option<i64>) -> PyResult<String> {
    consumption_state::integer_literal(py, value)
}
