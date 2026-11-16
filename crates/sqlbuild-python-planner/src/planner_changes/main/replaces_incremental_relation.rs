//! Incremental replacements, whose bounds must ignore the destination maximum.

use crate::planner_changes::_helpers::actions::CREATE_TABLE_ACTION;
use crate::planner_changes::_helpers::actions::INCREMENTAL_MATERIALIZATION;
use pyo3::pyfunction;

/// Incremental replacements, whose bounds must ignore the destination maximum.
#[pyfunction]
#[pyo3(signature = (*, materialization_type, action))]
pub fn replaces_incremental_relation(materialization_type: &str, action: &str) -> bool {
    materialization_type == INCREMENTAL_MATERIALIZATION && action == CREATE_TABLE_ACTION
}
