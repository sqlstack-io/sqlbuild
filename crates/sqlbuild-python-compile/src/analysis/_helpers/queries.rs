//! Validate, fingerprint and analyze SQL queries for the Python compiler.

use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;

#[pyfunction]
pub(crate) fn query_fingerprint(py: Python<'_>, sql: &str, dialect: &str) -> PyResult<String> {
    py.compiler_detach(|| {
        sqlbuild_analysis::sql_tokens::main::query_fingerprint::query_fingerprint(sql, dialect)
    })
    .map_err(value_error)
}

/// A warehouse type name's JSON type tree, None without a parser, `ValueError` on a panic.
#[pyfunction]
pub(crate) fn parse_warehouse_type(
    py: Python<'_>,
    spelling: &str,
    dialect: &str,
) -> PyResult<Option<String>> {
    py.compiler_detach(|| {
        sqlbuild_analysis::warehouse_types::main::parse_warehouse_type::parse_warehouse_type(
            spelling, dialect,
        )
        .map(|tree| tree.as_ref().map(serde_json::Value::to_string))
    })
    .map_err(value_error)
}
