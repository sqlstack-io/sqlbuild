//! Which runs a build re-reads while the warehouse is awake.

use sqlbuild_core::cost_command::constants::CHANGEABLE_LIMITATIONS;
use sqlbuild_core::cost_command::models::CostCoverage;

use crate::resource_costs::constants::{
    COLLECTION_FAILED_STATUS, MAX_FAILED_COLLECTION_ATTEMPTS, MAX_REFRESHED_RUNS, PARTIAL_STATUS,
    REFRESH_SETTLE_SECONDS, REFRESH_WINDOW_DAYS, REFRESHABLE_COST_STATUSES, SECONDS_PER_DAY,
};
use crate::resource_costs::models::RunCostAttempt;

/// Whether a partial run can still change: a statement unseen, running or lacking settling metadata.
fn partial_can_change(coverage: &CostCoverage) -> bool {
    coverage.observed_statement_count < coverage.expected_statement_count
        || coverage
            .limitations
            .iter()
            .any(|limitation| CHANGEABLE_LIMITATIONS.contains(&limitation.as_str()))
}

/// Newest settled runs a re-read can still change, skipping exhausted failures; `now` is epoch seconds.
#[must_use]
pub fn refresh_candidates(latest: &[RunCostAttempt], now: i64) -> Vec<RunCostAttempt> {
    let oldest: i64 = now - REFRESH_WINDOW_DAYS * SECONDS_PER_DAY;
    let mut candidates: Vec<RunCostAttempt> = latest
        .iter()
        .filter(|run| REFRESHABLE_COST_STATUSES.contains(&run.coverage.status.as_str()))
        .filter(|run| run.started_epoch >= oldest)
        .filter(|run| now - run.completed_epoch >= REFRESH_SETTLE_SECONDS)
        .filter(|run| run.coverage.status != PARTIAL_STATUS || partial_can_change(&run.coverage))
        .filter(|run| {
            run.coverage.status != COLLECTION_FAILED_STATUS
                || run.attempt < MAX_FAILED_COLLECTION_ATTEMPTS
        })
        .cloned()
        .collect();
    candidates.sort_by(|left, right| {
        right
            .started_epoch
            .cmp(&left.started_epoch)
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    candidates.truncate(MAX_REFRESHED_RUNS);
    candidates
}
