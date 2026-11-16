//! `fit_artifact_logical_name`, raising `SharedInputError` when the name cannot fit.

use crate::relation_naming::_helpers::fitting;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `fit_artifact_logical_name`, raising `SharedInputError` when the name cannot fit.
#[inline]
pub fn fitted_logical_name(
    py: Python<'_>,
    names: (&str, &str),
    identifier_limit: i64,
    artifact_label: &str,
) -> PyResult<String> {
    fitting::fitted_logical_name(py, names, identifier_limit, artifact_label)
}
