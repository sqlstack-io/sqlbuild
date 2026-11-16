//! Cost command errors raised as `CliUserError`.

use pyo3::prelude::{PyErr, Python};
use sqlbuild_core::cost_command::errors::CostCommandError;
use sqlbuild_python_base::records::main::construct::new_error::new_error;

use sqlbuild_python_base::records::types::ErrorId;

/// The `CliUserError` for a refused cost request.
pub(crate) fn cost_error(py: Python<'_>, error: &CostCommandError) -> PyErr {
    new_error(
        py,
        ErrorId::CliUser,
        &error.message,
        (Some(error.code), None),
    )
}
