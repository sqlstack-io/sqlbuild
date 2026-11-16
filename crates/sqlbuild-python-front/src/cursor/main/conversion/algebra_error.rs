//! `CursorAlgebraError(message)`.

use pyo3::prelude::{PyErr, Python};
use sqlbuild_host::entries::models::FacadeHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;

/// `CursorAlgebraError(message)`.
pub fn algebra_error(py: Python<'_>, message: &str) -> PyErr {
    host_error(py, FacadeHost::CursorAlgebraError, message)
}
