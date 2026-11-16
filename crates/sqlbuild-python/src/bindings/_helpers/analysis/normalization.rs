//! Normalize SQL for analysis and recover binding diagnostics.

use pyo3::prelude::{Bound, Py, PyAny, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::analysis::normalization_results::normalization_results;
use crate::bindings::models::Positional;
use sqlbuild_analysis::semantic_validation::models::DiagnosticRow;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;

#[pyfunction]
pub(crate) fn normalize_analysis_sql(
    py: Python<'_>,
    request: crate::bindings::models::NormalizationInput,
) -> PyResult<String> {
    py.compiler_detach(|| {
        sqlbuild_analysis::semantic_validation::main::normalize::normalize_analysis_sql(
            request.into(),
        )
    })
    .map_err(value_error)
}

#[pyfunction]
fn normalize_dialect_sql(py: Python<'_>, sql: &str, dialect: &str) -> PyResult<String> {
    py.compiler_detach(|| {
        sqlbuild_analysis::semantic_validation::main::normalize_dialect::normalize_dialect_sql(
            sql, dialect,
        )
    })
    .map_err(value_error)
}

/// Normalize a batch in one detached call; a failed member is returned as its exception.
#[pyfunction]
fn normalize_analysis_sqls(
    py: Python<'_>,
    dialect: &str,
    requests: Vec<sqlbuild_analysis::semantic_validation::types::NormalizationRequest>,
) -> PyResult<Vec<Py<PyAny>>> {
    let results = py.compiler_detach(|| {
        Ok(
            sqlbuild_analysis::semantic_validation::main::normalize_batch::normalize_analysis_sqls(
                dialect, requests, None,
            ),
        )
    });
    normalization_results(py, results.map_err(value_error)?)
}

#[pyfunction]
fn binding_diagnostics(
    py: Python<'_>,
    sql: &str,
    dialect: &str,
    rows: Vec<Positional<DiagnosticRow>>,
) -> PyResult<Vec<Positional<DiagnosticRow>>> {
    let rows: Vec<DiagnosticRow> = Positional::unwrap_all(rows);
    py.compiler_detach(|| {
        sqlbuild_analysis::semantic_validation::main::diagnostics::binding_diagnostics(
            sql, dialect, rows,
        )
    })
    .map(Positional::wrap_all)
    .map_err(value_error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(normalize_analysis_sql, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_dialect_sql, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_analysis_sqls, module)?)?;
    module.add_function(wrap_pyfunction!(binding_diagnostics, module)?)?;
    Ok(())
}
