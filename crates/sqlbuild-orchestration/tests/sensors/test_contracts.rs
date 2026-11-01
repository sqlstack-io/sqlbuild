//! Real stores prove atomic sensor consumption and recovery across scheduler ownership changes.

#[path = "helpers.rs"]
pub(crate) mod helpers;
#[path = "test_custom.rs"]
mod test_custom;
#[path = "test_delivery.rs"]
mod test_delivery;
#[path = "test_execution_bounds.rs"]
mod test_execution_bounds;
#[path = "test_nodes.rs"]
mod test_nodes;
#[path = "test_recovery.rs"]
mod test_recovery;
#[path = "test_runs.rs"]
mod test_runs;
#[path = "test_transitions.rs"]
mod test_transitions;
#[path = "test_types.rs"]
mod test_types;
