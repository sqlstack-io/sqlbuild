//! Run `body` inside one Python-node identity pass, so the identities it builds share source work.

use crate::native_host::_helpers::host;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Run `body` inside one Python-node identity pass, so the identities it builds share source work.
#[inline]
pub fn within_identity_pass<'py, T>(
    py: Python<'py>,
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    host::within_identity_pass(py, body)
}
