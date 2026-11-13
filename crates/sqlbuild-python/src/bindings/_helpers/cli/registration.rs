//! Register the native `sqb` argument parsing, command dispatch and command handlers.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::build_progress::{progress, progress_reporters, progress_text};
use crate::bindings::_helpers::cli::{
    argv, entry, launch_state, namespace, observability, termination,
};
use crate::bindings::_helpers::{
    audit_command, build_command, build_progress, change_selection, check, clone, command_runtime,
    compute_logs, contract_command, cost_command, cost_report, cost_telemetry, dbt_command,
    dbt_events, dbt_interop, debug_command, diff_report, event_exporting, execution_history,
    format_command, freshness, init_command, janitor, load, output_capture, plan_command,
    presentation_text, query_command, recall, result_output, review_command, scenario_command,
    seed_command, sql_test_command, workspace_commands,
};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    argv::register(module)?;
    entry::register(module)?;
    launch_state::register(module)?;
    namespace::register(module)?;
    observability::register(module)?;
    termination::register(module)?;
    progress::register(module)?;
    progress_reporters::register(module)?;
    progress_text::register(module)?;
    janitor::registration::register(module)?;
    plan_command::registration::register(module)?;
    freshness::registration::register(module)?;
    crate::bindings::_helpers::native_host::consumption_state::register(module)?;
    crate::bindings::_helpers::native_host::observations_state::register(module)?;
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
    dbt_events::registration::register(module)?;
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
