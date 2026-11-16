//! Register the macro modules functions and classes.

use crate::render::_helpers::expansion::macro_modules::inventory_macro_modules;
use crate::render::_helpers::expansion::macro_modules::macro_module_load_plan;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the macro modules functions and classes.
pub fn register_macro_modules(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(inventory_macro_modules, module)?)?;
    module.add_function(wrap_pyfunction!(macro_module_load_plan, module)?)?;
    Ok(())
}
