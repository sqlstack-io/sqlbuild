//! Validate, fingerprint and analyze SQL queries for the Python compiler.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::boundary::panics::value_error;
use crate::bindings::types::CompilerDetach;

#[pyfunction]
fn query_fingerprint(py: Python<'_>, sql: &str, dialect: &str) -> PyResult<String> {
    py.compiler_detach(|| {
        sqlbuild_analysis::sql_tokens::main::query_fingerprint::query_fingerprint(sql, dialect)
    })
    .map_err(value_error)
}

/// A warehouse type name's JSON type tree, None without a parser, `ValueError` on a panic.
#[pyfunction]
fn parse_warehouse_type(py: Python<'_>, spelling: &str, dialect: &str) -> PyResult<Option<String>> {
    py.compiler_detach(|| {
        sqlbuild_analysis::warehouse_types::main::parse_warehouse_type::parse_warehouse_type(
            spelling, dialect,
        )
        .map(|tree| tree.as_ref().map(serde_json::Value::to_string))
    })
    .map_err(value_error)
}

/// Captured Snowflake SQL as DuckDB SQL with its result columns, or None and SQLQuack's refusal.
#[pyfunction]
fn convert_local_scenario_sql(
    py: Python<'_>,
    sql: &str,
    schema_json: &str,
    name: &str,
) -> PyResult<sqlbuild_analysis::local_sql::types::LocalSqlParts> {
    py.compiler_detach(|| {
        sqlbuild_analysis::local_sql::main::convert_local_sql::convert_local_sql(
            sql,
            schema_json,
            name,
        )
    })
    .map(sqlbuild_analysis::local_sql::models::LocalSql::into_parts)
    .map_err(value_error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    crate::bindings::_helpers::analysis::binding_catalog::register(module)?;
    crate::bindings::_helpers::analysis::required_columns::register(module)?;
    crate::bindings::_helpers::references::explicit_references::register(module)?;
    crate::bindings::_helpers::analysis::binding_locations::register(module)?;
    module.add_function(wrap_pyfunction!(query_fingerprint, module)?)?;
    module.add_function(wrap_pyfunction!(parse_warehouse_type, module)?)?;
    module.add_function(wrap_pyfunction!(convert_local_scenario_sql, module)?)?;
    Ok(())
}
