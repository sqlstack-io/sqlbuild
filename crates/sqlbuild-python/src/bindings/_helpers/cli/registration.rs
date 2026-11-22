//! Register the native `sqb` argument parsing, command dispatch and command handlers.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::cli::interruption;
use crate::bindings::_helpers::cli::{argv, entry, namespace, observability, termination};
use crate::bindings::_helpers::{
    audit_command, build_command, build_progress, change_selection, check, clone, command_runtime,
    compute_logs, contract_command, cost_command, cost_report, cost_telemetry, dbt_command,
    dbt_interop, debug_command, diff_report, event_exporting, execution_history, format_command,
    freshness, init_command, janitor, load, output_capture, plan_command, presentation_text,
    query_command, recall, result_output, review_command, scenario_command, seed_command,
    sql_test_command, workspace_commands,
};
use sqlbuild_python_base::build_progress::main::progress::register_progress::register_progress;
use sqlbuild_python_base::cli::main::register_launch_state::register_launch_state;
use sqlbuild_python_output::build_progress::main::register_progress_reporters::register_progress_reporters;
use sqlbuild_python_output::build_progress::main::register_progress_text::register_progress_text;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    argv::register(module)?;
    entry::register(module)?;
    register_launch_state(module)?;
    namespace::register(module)?;
    observability::register(module)?;
    interruption::register(module)?;
    termination::register(module)?;
    register_progress(module)?;
    register_progress_reporters(module)?;
    register_progress_text(module)?;
    janitor::registration::register(module)?;
    plan_command::registration::register(module)?;
    freshness::registration::register(module)?;
    sqlbuild_python_front::native_host::main::register_consumption_state::register_consumption_state(module)?;
    sqlbuild_python_front::native_host::main::register_observations_state::register_observations_state(module)?;
    sqlbuild_python_front::resource_costs::main::register_resource_costs_state::register_resource_costs_state(module)?;
    sqlbuild_python_front::native_host::main::register_audit_results_state::register_audit_results_state(module)?;
    sqlbuild_python_project::native_host::main::register_runs_state::register_runs_state(module)?;
    clone::registration::register(module)?;
    compute_logs::registration::register(module)?;
    execution_history::registration::register(module)?;
    output_capture::registration::register(module)?;
    event_exporting::registration::register(module)?;
    load::registration::register(module)?;
    result_output::registration::register(module)?;
    cost_report::registration::register(module)?;
    cost_telemetry::registration::register(module)?;
    debug_command::registration::register(module)?;
    sqlbuild_python_output::dbt_events::main::register_dbt_events::register_dbt_events(module)?;
    dbt_interop::registration::register(module)?;
    diff_report::registration::register(module)?;
    review_command::registration::register(module)?;
    command_runtime::registration::register(module)?;
    presentation_text::registration::register(module)?;
    workspace_commands::registration::register(module)?;
    check::registration::register(module)?;
    sql_test_command::registration::register(module)?;
    change_selection::registration::register(module)?;
    seed_command::registration::register(module)?;
    audit_command::registration::register(module)?;
    build_command::registration::register(module)?;
    build_progress::registration::register(module)?;
    query_command::registration::register(module)?;
    recall::registration::register(module)?;
    scenario_command::registration::register(module)?;
    contract_command::registration::register(module)?;
    init_command::registration::register(module)?;
    format_command::registration::register(module)?;
    cost_command::registration::register(module)?;
    dbt_command::registration::register(module)?;
    Ok(())
}
