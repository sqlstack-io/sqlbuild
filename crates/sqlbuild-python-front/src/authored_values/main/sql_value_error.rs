//! The exception a normalization failure raises.

use crate::authored_values::_helpers::sql_values;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_model_config::sql_values::errors::SqlValueFailure;

/// The exception a normalization failure raises.
#[inline]
pub fn sql_value_error(py: Python<'_>, failure: SqlValueFailure) -> PyResult<PyErr> {
    sql_values::sql_value_error(py, failure)
}
