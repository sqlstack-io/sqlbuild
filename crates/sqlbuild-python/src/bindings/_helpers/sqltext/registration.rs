//! Register the lexical SQL text functions and classes.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_front::sqltext::main::register_code_scanner::register_code_scanner;
use sqlbuild_python_front::sqltext::main::register_ddl_effects::register_ddl_effects;
use sqlbuild_python_front::sqltext::main::register_model_headers::register_model_headers;
use sqlbuild_python_front::sqltext::main::register_placeholders::register_placeholders;
use sqlbuild_python_front::sqltext::main::register_sql_references::register_sql_references;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_model_headers(module)?;
    register_code_scanner(module)?;
    register_sql_references(module)?;
    register_placeholders(module)?;
    register_ddl_effects(module)?;
    Ok(())
}
