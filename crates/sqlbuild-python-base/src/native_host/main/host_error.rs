//! The host exception class `id` raised with `message`, or the error looking it up or raising it.

use crate::native_host::_helpers::host::entry;
use crate::records::main::construct::exception_from::exception_from;
use pyo3::PyErr;
use pyo3::prelude::Python;
use sqlbuild_host::entries::models::HostId;

/// The host exception class `id` raised with `message`, or the error looking it up or raising it.
pub fn host_error(py: Python<'_>, id: impl Into<HostId>, message: &str) -> PyErr {
    exception_from(entry(py, id), message, (None, None))
}
