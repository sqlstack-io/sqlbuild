//! Register the strategy functions and classes.

use crate::planner_strategy::_helpers::strategy::build_model_warnings;
use crate::planner_strategy::_helpers::strategy::resolve_schema_actions;
use crate::planner_strategy::main::strategy::materialization_of::materialization_of;
use crate::planner_strategy::main::strategy::resolve_model_plan_action::resolve_model_plan_action;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the strategy functions and classes.
pub fn register_strategy(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(materialization_of, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_model_plan_action, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_schema_actions, module)?)?;
    module.add_function(wrap_pyfunction!(build_model_warnings, module)?)
}
