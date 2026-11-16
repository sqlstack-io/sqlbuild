//! Register the guidance functions and classes.

use crate::rules::main::configuration::format_rule::format_rule;
use crate::rules::main::configuration::install_rules_skills::install_rules_skills;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the guidance functions and classes.
pub fn register_guidance(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_rule, module)?)?;
    module.add_function(wrap_pyfunction!(install_rules_skills, module)?)?;
    Ok(())
}
