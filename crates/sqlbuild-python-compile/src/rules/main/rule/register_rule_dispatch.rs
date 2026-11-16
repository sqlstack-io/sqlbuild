//! Register the rule dispatch functions and classes.

use crate::rules::_helpers::rule::rule_dispatch::evaluate_custom_rules;
use crate::rules::_helpers::rule::rule_dispatch::evaluate_native_rules;
use crate::rules::_helpers::rule::rule_dispatch::evaluate_project_rules;
use crate::rules::_helpers::rule::rule_dispatch::evaluate_rules;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the rule dispatch functions and classes.
pub fn register_rule_dispatch(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(evaluate_project_rules, module)?)?;
    module.add_function(wrap_pyfunction!(evaluate_native_rules, module)?)?;
    module.add_function(wrap_pyfunction!(evaluate_custom_rules, module)?)?;
    module.add_function(wrap_pyfunction!(evaluate_rules, module)?)
}
