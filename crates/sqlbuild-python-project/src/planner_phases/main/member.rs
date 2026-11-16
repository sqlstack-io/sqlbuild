//! A staying-Python enum member from its value.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::entries::models::HostId;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// A staying-Python enum member from its value.
pub fn member<'py>(
    py: Python<'py>,
    class: impl Into<HostId>,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    host_entry(py, class)?.call1((value,))
}
