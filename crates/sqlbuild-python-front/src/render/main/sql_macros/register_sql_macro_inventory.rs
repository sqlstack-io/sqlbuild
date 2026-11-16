//! Register the sql macro inventory functions and classes.

use crate::render::_helpers::sql_macros::sql_macro_inventory::inventory_sql_macros;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the sql macro inventory functions and classes.
pub fn register_sql_macro_inventory(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(inventory_sql_macros, module)?)
}
