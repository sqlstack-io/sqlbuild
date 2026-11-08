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

/// The type tree, as JSON, of a warehouse type name, or None when `dialect` has no parser.
///
/// A parser panic is a `ValueError`, so capture falls back as for any rejected spelling.
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
    crate::bindings::_helpers::analysis::binding_catalog::register(module)?;
    crate::bindings::_helpers::analysis::required_columns::register(module)?;
    crate::bindings::_helpers::references::explicit_references::register(module)?;
    crate::bindings::_helpers::analysis::binding_locations::register(module)?;
    module.add_function(wrap_pyfunction!(query_fingerprint, module)?)?;
    module.add_function(wrap_pyfunction!(parse_warehouse_type, module)?)?;
    Ok(())
}
