//! Warehouse state rows SQLBuild appends: event IDs, event SQL and source freshness versions.

#![forbid(unsafe_code)]

pub mod consumption;
pub mod events;
pub mod execution_history;
pub mod freshness;
pub mod microbatches;
pub mod observations;
pub mod state_tables;
