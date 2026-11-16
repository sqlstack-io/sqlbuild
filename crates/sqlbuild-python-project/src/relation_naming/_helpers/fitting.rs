//! Generated relation names fitted within identifier limits, raising `SharedInputError`.

use crate::relation_naming::main::fitted_logical_name::fitted_logical_name;
use crate::relation_naming::main::shared_error::shared_error;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_analysis::identifier_fitting::main::fit_auxiliary_relation_name::fit_auxiliary_relation_name as fit_auxiliary;

use sqlbuild_python_base::boundary::models::Request;

/// A readable logical component fitted after a fixed prefix, with a hash suffix when shortened.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn fit_artifact_logical_name(options: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let request = Request::new("fit_artifact_logical_name", options)?;
    fitted_logical_name(
        request.py(),
        (
            &request.get("logical_name")?.extract::<String>()?,
            &request.get("fixed_prefix")?.extract::<String>()?,
        ),
        request.get("identifier_limit")?.extract()?,
        &request.get("artifact_label")?.extract::<String>()?,
    )
}

/// `base_name + suffix` fitted within the limit, keeping the suffix and a base hash.
#[pyfunction]
#[pyo3(signature = (*, base_name, suffix, identifier_limit))]
pub(crate) fn fit_auxiliary_relation_name(
    py: Python<'_>,
    base_name: &str,
    suffix: &str,
    identifier_limit: i64,
) -> PyResult<String> {
    fit_auxiliary(base_name, suffix, identifier_limit).map_err(|message| shared_error(py, message))
}
