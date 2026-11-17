//! Register the planner strategy.

use pyo3::prelude::{Bound, PyModule, PyResult};
use pyo3::{prelude::PyModuleMethods, wrap_pyfunction};

use crate::bindings::_helpers::planner_strategy::audits;
use crate::bindings::_helpers::planner_strategy::cursor_types;
use crate::bindings::_helpers::planner_strategy::microbatches;
use crate::bindings::_helpers::planner_strategy::strategy;
use sqlbuild_python_project::planner_strategy::main::register_fixture_completion::register_fixture_completion;
use sqlbuild_python_project::planner_strategy::main::register_fixture_context::register_fixture_context;
use sqlbuild_python_project::planner_strategy::main::register_fixture_reads::register_fixture_reads;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_strategy::audit_entries::plan_audit_of,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_strategy::cursor_inputs::cursor_input_columns_of,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_strategy::cursor_inputs::cursor_input_relations_of,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_strategy::cursor_inputs::runtime_cursor_producer_names,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_strategy::model_entries::resolve_microbatch_lookback,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_strategy::microbatch_limits::apply_microbatch_limit,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        crate::bindings::_helpers::planner_strategy::microbatch_limits::apply_configured_microbatch_limits,
        module
    )?)?;
    audits::register(module)?;
    cursor_types::register(module)?;
    register_fixture_completion(module)?;
    register_fixture_context(module)?;
    register_fixture_reads(module)?;
    microbatches::register(module)?;
    strategy::register(module)
}
