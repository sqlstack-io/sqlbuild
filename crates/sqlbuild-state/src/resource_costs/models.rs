//! The latest collection of a run, as its run row records it.

use sqlbuild_core::cost_command::models::CostCoverage;

/// A run row's identity and coverage: the newest attempt per run decides what a refresh does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunCostAttempt {
    pub run_id: String,
    /// The server run a location build ran for; None for a build run from the command line.
    pub orchestration_run_id: Option<String>,
    pub attempt: i64,
    pub coverage: CostCoverage,
    pub target_name: Option<String>,
    /// ISO 8601 texts of the run's window, as recorded.
    pub run_started_at: String,
    pub run_completed_at: String,
    /// Seconds since the epoch of the run's start.
    pub started_epoch: i64,
}

/// What identifies one observation: the run, the collection attempt, the resource and its warehouse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceCostKey<'a> {
    pub run_id: &'a str,
    pub attempt: i64,
    pub resource_type: &'a str,
    pub resource_name: &'a str,
    pub warehouse_name: &'a str,
    pub warehouse_size: &'a str,
    pub warehouse_type: Option<&'a str>,
    pub cluster_number: Option<i64>,
}
