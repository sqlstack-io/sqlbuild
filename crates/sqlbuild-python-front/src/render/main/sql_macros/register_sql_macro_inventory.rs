//! Register the sql macro inventory functions and classes.

use crate::render::_helpers::sql_macros::sql_macro_inventory;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the sql macro inventory functions and classes.
#[inline]
pub fn register_sql_macro_inventory(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sql_macro_inventory::register(module)
}
