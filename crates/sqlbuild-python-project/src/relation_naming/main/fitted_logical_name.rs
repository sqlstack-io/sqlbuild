//! `fit_artifact_logical_name`, raising `SharedInputError` when the name cannot fit.

use crate::relation_naming::main::shared_error::shared_error;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::identifier_fitting::main::fit_artifact_logical_name::fit_artifact_logical_name as fit_logical;

/// `fit_artifact_logical_name`, raising `SharedInputError` when the name cannot fit.
pub fn fitted_logical_name(
    py: Python<'_>,
    names: (&str, &str),
    identifier_limit: i64,
    artifact_label: &str,
) -> PyResult<String> {
    fit_logical(names, identifier_limit, artifact_label)
        .map_err(|message| shared_error(py, message))
}
