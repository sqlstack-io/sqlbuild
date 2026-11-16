//! Validate, fingerprint and analyze SQL queries for the Python compiler.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;

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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sqlbuild_python_project::analysis::main::register_binding_catalog::register_binding_catalog(
        module,
    )?;
    crate::bindings::_helpers::analysis::required_columns::register(module)?;
    sqlbuild_python_project::references::main::register_explicit_references::register_explicit_references(module)?;
    crate::bindings::_helpers::analysis::binding_locations::register(module)?;
    module.add_function(wrap_pyfunction!(query_fingerprint, module)?)?;
    module.add_function(wrap_pyfunction!(parse_warehouse_type, module)?)?;
    Ok(())
}
