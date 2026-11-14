//! Run attempts and observation IDs shared by the resource cost tests.

use sqlbuild_core::cost_command::models::CostCoverage;

use crate::events::main::resource_cost_observation_id::resource_cost_observation_id;
use crate::resource_costs::models::{ResourceCostKey, RunCostAttempt};

/// The fixed clock every refresh case is judged at, in epoch seconds.
pub(crate) const NOW: i64 = 1_800_000_000;

pub(crate) fn run(run_id: &str, attempt: i64, status: &str, age: i64) -> RunCostAttempt {
    RunCostAttempt {
        run_id: run_id.to_owned(),
        orchestration_run_id: None,
        attempt,
        coverage: CostCoverage {
            status: status.to_owned(),
            expected_statement_count: 2,
            observed_statement_count: 1,
            query_count: 1,
            limitations: Vec::new(),
        },
        target_name: None,
        run_started_at: String::new(),
        run_completed_at: String::new(),
        started_epoch: NOW - age,
    }
}

pub(crate) fn observation_id(attempt: i64, cluster_number: Option<i64>) -> String {
    resource_cost_observation_id(&ResourceCostKey {
        run_id: "run",
        attempt,
        resource_type: "model",
        resource_name: "orders",
        warehouse_name: "BUILD_WH",
        warehouse_size: "X-Small",
        warehouse_type: Some("STANDARD"),
        cluster_number,
    })
    .expect("identity encodes")
}
