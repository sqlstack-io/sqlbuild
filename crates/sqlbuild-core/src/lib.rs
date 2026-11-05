//! Shared foundations of the SQLBuild native workspace: identities, diagnostics, text and JSON.

#![forbid(unsafe_code)]

pub mod audit_command;
pub mod build_aggregate;
pub mod build_command;
pub mod build_progress;
pub mod check_command;
pub mod cli_text;
pub mod clone_text;
pub mod command_output;
pub mod compile_timings;
pub mod constants;
pub mod contract_command;
pub mod cost_allocation;
pub mod cost_command;
pub mod cost_report;
pub mod dagster_invocation;
pub mod dbt_cli;
pub mod dbt_command;
pub mod dbt_events;
pub mod dbt_graph;
pub mod dbt_plan;
pub mod dbt_process;
pub mod debug_report;
pub mod diagnostic_routing;
pub mod diagnostic_text;
pub mod dlt_destination;
pub mod event_export;
pub mod fingerprint_identity;
pub mod format_command;
pub mod freshness_text;
pub mod git_revisions;
pub mod graph_walk;
pub mod identifier_text;
pub mod ingestr_command;
pub mod init_command;
pub mod integration_results;
pub mod janitor_text;
pub mod json;
pub mod lifecycle_events;
pub mod load_command;
pub mod loader_connection;
pub mod migration_values;
pub mod models;
pub mod nested_progress;
pub mod panics;
pub mod plan_display;
pub mod posix_path;
pub mod progress;
pub mod query_command;
pub mod redaction;
pub mod scenario_command;
pub mod seed_command;
pub mod shell_text;
pub mod terminal_events;
pub mod test_command;
pub mod text;

#[cfg(test)]
mod tests;
