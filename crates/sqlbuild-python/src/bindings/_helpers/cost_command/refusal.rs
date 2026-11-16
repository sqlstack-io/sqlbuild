//! Cost command errors raised as `CliUserError`.

use pyo3::prelude::{Bound, PyAnyMethods, PyErr, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_core::cost_command::errors::CostCommandError;

use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// The `CliUserError` for a refused cost request.
pub(crate) fn cost_error(py: Python<'_>, error: &CostCommandError) -> PyErr {
    let raised = (|| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("code", error.code)?;
        error_class(py, ErrorId::CliUser)?.call((error.message.as_str(),), Some(&options))
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(failure) => failure,
    }
}
