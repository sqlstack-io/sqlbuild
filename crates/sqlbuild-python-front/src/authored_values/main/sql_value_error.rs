//! The exception a normalization failure raises.

use pyo3::prelude::{PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_model_config::sql_values::errors::SqlValueFailure;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::NativeAttr;

/// The exception a normalization failure raises.
pub fn sql_value_error(py: Python<'_>, failure: SqlValueFailure) -> PyResult<PyErr> {
    Ok(match failure {
        SqlValueFailure::Invalid(message) => PyErr::from_value(
            native_attr(py, NativeAttr::SqlValueValidationError)?.call1((message,))?,
        ),
        SqlValueFailure::DecimalOverflow => {
            let overflow = py.import("decimal")?.getattr("Overflow")?;
            let classes = pyo3::types::PyList::new(py, [overflow.clone()])?;
            PyErr::from_value(overflow.call1((classes,))?)
        }
    })
}
