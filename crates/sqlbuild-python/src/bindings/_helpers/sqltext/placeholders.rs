//! Substitute `@@@name` placeholder defaults for SQL analysis.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

/// Python's `substitute_placeholder_defaults`: each `@@@name` becomes its default.
#[pyfunction]
fn substitute_placeholder_defaults(
    py: Python<'_>,
    sql: &str,
    placeholders: Vec<(String, String)>,
) -> PyResult<String> {
    py.compiler_detach(|| Ok(placeholder_defaults(sql, &placeholders)))
        .map_err(compiler_error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(substitute_placeholder_defaults, module)?)?;
    Ok(())
}
