//! Which runs a build re-reads while the warehouse is awake.

use crate::resource_costs::constants::{
    COLLECTION_FAILED_STATUS, MAX_FAILED_COLLECTION_ATTEMPTS, MAX_REFRESHED_RUNS,
    REFRESH_WINDOW_DAYS, REFRESHABLE_COST_STATUSES,
};
use crate::resource_costs::models::RunCostAttempt;

const SECONDS_PER_DAY: i64 = 86_400;

/// Newest first, at most `MAX_REFRESHED_RUNS`: refreshable runs inside the query history window,
/// except failed collections that already used every attempt. `now` is seconds since the epoch.
#[must_use]
pub fn refresh_candidates(latest: &[RunCostAttempt], now: i64) -> Vec<RunCostAttempt> {
    let oldest: i64 = now - REFRESH_WINDOW_DAYS * SECONDS_PER_DAY;
    let mut candidates: Vec<RunCostAttempt> = latest
        .iter()
        .filter(|run| REFRESHABLE_COST_STATUSES.contains(&run.coverage.status.as_str()))
        .filter(|run| run.started_epoch >= oldest)
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
