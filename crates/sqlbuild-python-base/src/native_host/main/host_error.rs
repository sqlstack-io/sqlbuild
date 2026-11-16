//! The host exception class `id` raised with `message`, or the error looking it up or raising it.

use crate::native_host::_helpers::host;
use pyo3::PyErr;
use pyo3::prelude::Python;
use sqlbuild_host::entries::models::HostId;

/// The host exception class `id` raised with `message`, or the error looking it up or raising it.
#[inline]
pub fn host_error(py: Python<'_>, id: impl Into<HostId>, message: &str) -> PyErr {
    host::host_error(py, id, message)
}
