//! A finished run's status from how it ended and its node counts, by the server's rule for its own runs.

use crate::runs::constants::{CANCELLED_STATUS, FAILED_STATUS, SUCCEEDED_STATUS};
use crate::runs::models::{RunEnding, RunNodeCounts};

/// `cancelled` if interrupted, else `failed` on an error or a node that failed, was skipped or never started, else `succeeded`.
#[must_use]
pub fn run_status(counts: &RunNodeCounts, ending: RunEnding) -> &'static str {
    let bad: bool = counts.failed + counts.skipped + counts.not_started > 0;
    match (ending, bad) {
        (RunEnding::Interrupted, _) => CANCELLED_STATUS,
        (RunEnding::Raised, _) | (RunEnding::Completed, true) => FAILED_STATUS,
        (RunEnding::Completed, false) => SUCCEEDED_STATUS,
    }
}
