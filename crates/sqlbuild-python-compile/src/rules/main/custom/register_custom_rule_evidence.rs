//! Register the custom rule evidence functions and classes.

use crate::rules::_helpers::custom::custom_rule_evidence::allowed_custom_rule_import_binding;
use crate::rules::_helpers::custom::custom_rule_evidence::custom_rule_implementation_fingerprint;
use crate::rules::_helpers::custom::custom_rule_evidence::custom_rule_import_closure;
use crate::rules::_helpers::custom::custom_rule_evidence::custom_rule_test_evidence;
use crate::rules::_helpers::custom::custom_rule_evidence::verify_custom_rules;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the custom rule evidence functions and classes.
pub fn register_custom_rule_evidence(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        allowed_custom_rule_import_binding,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(verify_custom_rules, module)?)?;
    module.add_function(wrap_pyfunction!(custom_rule_import_closure, module)?)?;
    module.add_function(wrap_pyfunction!(
        custom_rule_implementation_fingerprint,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(custom_rule_test_evidence, module)?)?;
    Ok(())
}
