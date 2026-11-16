//! Register the selection phases functions and classes.

use crate::compile_driver::_helpers::stages::selection_phases::resolve_analysis_model_names;
use crate::compile_driver::main::stages::apply_unit_test_selectors::apply_unit_test_selectors;
use crate::compile_driver::main::stages::resolve_configured_rule_selection::resolve_configured_rule_selection;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the selection phases functions and classes.
pub fn register_selection_phases(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(apply_unit_test_selectors, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_configured_rule_selection, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_analysis_model_names, module)?)?;
    Ok(())
}
