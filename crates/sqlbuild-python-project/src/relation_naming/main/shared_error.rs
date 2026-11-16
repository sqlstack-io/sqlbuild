//! Shared error.

use pyo3::prelude::{PyErr, Python};
use sqlbuild_host::entries::models::FacadeHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;

/// Shared error.
pub fn shared_error(py: Python<'_>, message: String) -> PyErr {
    host_error(py, FacadeHost::SharedInputError, &message)
}
