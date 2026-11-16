//! The staying-Python class `class(**fields)`.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_host::entries::models::HostId;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// The staying-Python class `class(**fields)`.
pub fn construct<'py>(
    py: Python<'py>,
    class: impl Into<HostId>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    call_with(&host_entry(py, class)?, fields)
}
