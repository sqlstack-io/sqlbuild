//! Register the catalogue functions and classes.

use crate::rules::_helpers::configuration::catalogue::resolve_rule_ignore_selectors;
use crate::rules::_helpers::configuration::catalogue::select_catalogue_rules;
use crate::rules::main::configuration::build_rule_catalogue::build_rule_catalogue;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the catalogue functions and classes.
pub fn register_catalogue(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_rule_catalogue, module)?)?;
    module.add_function(wrap_pyfunction!(select_catalogue_rules, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_rule_ignore_selectors, module)?)?;
    Ok(())
}
