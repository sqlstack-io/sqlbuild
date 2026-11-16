//! Register the rule run functions and classes.

use crate::rules::_helpers::rule::rule_run::finish_rule_run;
use crate::rules::_helpers::rule::rule_run::rules_bulk_cache_path;
use crate::rules::main::rule::run_rules::run_rules;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the rule run functions and classes.
pub fn register_rule_run(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(finish_rule_run, module)?)?;
    module.add_function(wrap_pyfunction!(run_rules, module)?)?;
    module.add_function(wrap_pyfunction!(rules_bulk_cache_path, module)?)?;
    Ok(())
}
