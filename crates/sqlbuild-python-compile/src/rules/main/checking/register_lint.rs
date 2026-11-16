//! Register the lint functions and classes.

use crate::rules::_helpers::checking::lint::format_sql_json;
use crate::rules::_helpers::checking::lint::lint_sql_json;
use crate::rules::_helpers::checking::lint::prepare_lint_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;
use sqlbuild_python_project::compiled_views::main::compiled_models::register_relation_keys::register_relation_keys;

/// Register the lint functions and classes.
pub fn register_lint(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(lint_sql_json, module)?)?;
    module.add_function(wrap_pyfunction!(prepare_lint_sql, module)?)?;
    module.add_function(wrap_pyfunction!(format_sql_json, module)?)?;
    register_relation_keys(module)
}
