//! Normalize analysis sql.

use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;

#[pyfunction]
pub fn normalize_analysis_sql(
    py: Python<'_>,
    request: crate::analysis::models::NormalizationInput,
) -> PyResult<String> {
    py.compiler_detach(|| {
        sqlbuild_analysis::semantic_validation::main::normalize::normalize_analysis_sql(
            request.into(),
        )
    })
    .map_err(value_error)
}
