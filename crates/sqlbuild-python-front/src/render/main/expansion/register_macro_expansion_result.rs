//! Register the macro expansion result functions and classes.

use crate::render::_helpers::expansion::macro_expansion_result::expand_sql_macros;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the macro expansion result functions and classes.
pub fn register_macro_expansion_result(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(expand_sql_macros, module)?)
}
