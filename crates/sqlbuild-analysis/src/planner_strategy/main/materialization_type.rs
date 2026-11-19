//! The materialization type of a model's `materialized` value.

use crate::planner_strategy::models::MaterializationType;

/// A table when unset; an unknown string is a custom materialization.
pub fn materialization_type(materialized: Option<&str>) -> MaterializationType {
    match materialized {
        None | Some("table") => MaterializationType::Table,
        Some("view") => MaterializationType::View,
        Some("incremental") => MaterializationType::Incremental,
        Some("snapshot") => MaterializationType::Snapshot,
        Some("seed") => MaterializationType::Seed,
        Some("dynamic") => MaterializationType::Dynamic,
        Some(_) => MaterializationType::Custom,
    }
}
