//! Register the rule api functions and classes.

use crate::rules::_helpers::rule::rule_api::finding_position;
use crate::rules::_helpers::rule::rule_api::project_tree_paths;
use crate::rules::_helpers::rule::rule_api::rule_option_toml_value;
use crate::rules::_helpers::rule::rule_api::validate_project_relative;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the rule api functions and classes.
pub fn register_rule_api(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(finding_position, module)?)?;
    module.add_function(wrap_pyfunction!(rule_option_toml_value, module)?)?;
    module.add_function(wrap_pyfunction!(validate_project_relative, module)?)?;
    module.add_function(wrap_pyfunction!(project_tree_paths, module)?)?;
    Ok(())
}
