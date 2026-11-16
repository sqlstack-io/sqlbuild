//! Register the planner strategy.

use pyo3::prelude::PyModuleMethods;
use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_planner::planner_strategy::main::audits::register_audits::register_audits;
use sqlbuild_python_planner::planner_strategy::main::cursor_types::register_cursor_types::register_cursor_types;
use sqlbuild_python_planner::planner_strategy::main::microbatches::register_microbatches::register_microbatches;
use sqlbuild_python_planner::planner_strategy::main::strategy::register_strategy::register_strategy;
use sqlbuild_python_project::planner_strategy::main::register_fixture_completion::register_fixture_completion;
use sqlbuild_python_project::planner_strategy::main::register_fixture_context::register_fixture_context;
use sqlbuild_python_project::planner_strategy::main::register_fixture_reads::register_fixture_reads;
use sqlbuild_python_planner::planner_strategy::main::audit_entries::plan_audit_of_function::plan_audit_of_function;
use sqlbuild_python_planner::planner_strategy::main::cursor_inputs::cursor_input_columns_of_function::cursor_input_columns_of_function;
use sqlbuild_python_planner::planner_strategy::main::cursor_inputs::cursor_input_relations_of_function::cursor_input_relations_of_function;
use sqlbuild_python_planner::planner_strategy::main::cursor_inputs::runtime_cursor_producer_names_function::runtime_cursor_producer_names_function;
use sqlbuild_python_planner::planner_strategy::main::microbatch_limits::apply_configured_microbatch_limits_function::apply_configured_microbatch_limits_function;
use sqlbuild_python_planner::planner_strategy::main::microbatch_limits::apply_microbatch_limit_function::apply_microbatch_limit_function;
use sqlbuild_python_planner::planner_strategy::main::model_entries::resolve_microbatch_lookback_function::resolve_microbatch_lookback_function;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(plan_audit_of_function(module)?)?;
    module.add_function(cursor_input_columns_of_function(module)?)?;
    module.add_function(cursor_input_relations_of_function(module)?)?;
    module.add_function(runtime_cursor_producer_names_function(module)?)?;
    module.add_function(resolve_microbatch_lookback_function(module)?)?;
    module.add_function(apply_microbatch_limit_function(module)?)?;
    module.add_function(apply_configured_microbatch_limits_function(module)?)?;
    register_audits(module)?;
    register_cursor_types(module)?;
    register_fixture_completion(module)?;
    register_fixture_context(module)?;
    register_fixture_reads(module)?;
    register_microbatches(module)?;
    register_strategy(module)
}
