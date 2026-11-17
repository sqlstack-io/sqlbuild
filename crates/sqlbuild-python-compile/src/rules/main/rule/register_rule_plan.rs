//! Register the rule plan functions and classes.

use crate::rules::_helpers::rule::rule_plan::plan_rule_run;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the rule plan functions and classes.
pub fn register_rule_plan(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(plan_rule_run, module)?)?;
    Ok(())
}
