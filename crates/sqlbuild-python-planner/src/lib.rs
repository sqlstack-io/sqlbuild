//! The plan layer of the `sqlbuild._native` bindings: execution plans and their entries, planner phases, changes, migrations, resolution, scenarios, strategy and warehouse state, freshness, consumption, microbatches, observations, lifecycle events and plan output.

#![forbid(unsafe_code)]

pub mod consumption;
pub mod freshness;
pub mod lifecycle_events;
pub mod live_reads;
pub mod microbatches;
pub mod observations;
pub mod plan_output;
pub mod planner_changes;
pub mod planner_entries;
pub mod planner_identity;
pub mod planner_migrations;
pub mod planner_phases;
pub mod planner_resolution;
pub mod planner_scenario;
pub mod planner_strategy;
pub mod planner_warehouse;
