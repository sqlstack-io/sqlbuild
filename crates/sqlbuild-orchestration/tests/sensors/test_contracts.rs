//! Real stores prove atomic sensor consumption and recovery across scheduler ownership changes.

#[path = "helpers.rs"]
pub(crate) mod helpers;
#[path = "test_checks.rs"]
mod test_checks;
#[path = "test_custom.rs"]
mod test_custom;
#[path = "test_delivery.rs"]
mod test_delivery;
#[path = "test_execution_bounds.rs"]
mod test_execution_bounds;
#[path = "test_failing.rs"]
mod test_failing;
#[path = "test_function_cursor.rs"]
mod test_function_cursor;
#[path = "test_group_members.rs"]
mod test_group_members;
#[path = "test_nodes.rs"]
mod test_nodes;
#[path = "test_permissions.rs"]
mod test_permissions;
#[path = "test_person_state.rs"]
mod test_person_state;
#[path = "test_recovery.rs"]
mod test_recovery;
#[path = "test_run_keys.rs"]
mod test_run_keys;
#[path = "test_runs.rs"]
mod test_runs;
#[path = "test_skip.rs"]
mod test_skip;
#[path = "test_sources.rs"]
mod test_sources;
#[path = "test_transitions.rs"]
mod test_transitions;
#[path = "test_types.rs"]
mod test_types;
#[path = "test_upgrade.rs"]
mod test_upgrade;
