//! Public traits and type aliases of result output.

use pyo3::prelude::PyResult;
use sqlbuild_core::integration_results::errors::IntegrationRecordError;

/// A contract failure, or a Python failure while reading the values.
pub type Checked<T> = PyResult<Result<T, IntegrationRecordError>>;
