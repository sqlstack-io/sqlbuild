//! The Python exception a failed project validation raises.

use crate::discovery::_helpers::project::project_validation;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;
use sqlbuild_discovery::project_validation::models::ValidationFailure;

/// The Python exception a failed project validation raises.
#[inline]
pub fn validation_error(py: Python<'_>, failure: ValidationFailure) -> PyErr {
    project_validation::validation_error(py, failure)
}
