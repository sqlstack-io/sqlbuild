//! Register the native semantic completion functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_compile::semantic_checks::main::register_help_catalogue::register_help_catalogue;
use sqlbuild_python_compile::semantic_checks::main::register_resource_sql::register_resource_sql;
use sqlbuild_python_compile::semantic_checks::main::register_stage::register_stage;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_resource_sql(module)?;
    register_help_catalogue(module)?;
    register_stage(module)
}
