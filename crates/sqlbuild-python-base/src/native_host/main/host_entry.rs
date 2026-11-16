//! The host entry `id`: its target module imported and the attribute read on every call.

use crate::native_host::_helpers::host;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_host::entries::models::HostId;

/// The host entry `id`: its target module imported and the attribute read on every call.
#[inline]
pub fn host_entry<'py>(py: Python<'py>, id: impl Into<HostId>) -> PyResult<Bound<'py, PyAny>> {
    host::entry(py, id)
}
