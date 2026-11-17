//! One call's cursor values read from and built as the native cursor value records.

use sqlbuild_python_base::native_host::main::host_error::host_error;

use pyo3::prelude::{PyErr, Python};
use sqlbuild_host::entries::models::FacadeHost;

/// `CursorAlgebraError(message)`.
pub(crate) fn algebra_error(py: Python<'_>, message: &str) -> PyErr {
    host_error(py, FacadeHost::CursorAlgebraError, message)
}
