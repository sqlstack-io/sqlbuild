//! `compiled_code_identity`, looked up on the extension module so tests can replace it.

use crate::cache::_helpers::native_store;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `compiled_code_identity`, looked up on the extension module so tests can replace it.
#[inline]
pub fn current_code_identity(py: Python<'_>) -> PyResult<String> {
    native_store::current_code_identity(py)
}
