//! Register the lint sql bodies functions and classes.

use crate::rules::_helpers::lint::lint_sql_bodies::lint_native_sql_bodies;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the lint sql bodies functions and classes.
pub fn register_lint_sql_bodies(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(lint_native_sql_bodies, module)?)
}
