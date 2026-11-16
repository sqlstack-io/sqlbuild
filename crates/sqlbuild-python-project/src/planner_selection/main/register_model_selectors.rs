//! Register the model selectors functions and classes.

use crate::planner_selection::main::split_selector_expansion::split_selector_expansion;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the model selectors functions and classes.
pub fn register_model_selectors(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(split_selector_expansion, module)?)?;
    Ok(())
}
