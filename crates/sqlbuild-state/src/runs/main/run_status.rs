//! A finished run's status from how it ended and its node counts, by the server's rule for its own runs.

use crate::runs::constants::{CANCELLED_STATUS, FAILED_STATUS, PARTIAL_STATUS, SUCCEEDED_STATUS};
use crate::runs::models::{RunEnding, RunNodeCounts};

/// `cancelled` if interrupted, `failed` on an error, else partial, failed or succeeded by node counts.
#[must_use]
pub fn run_status(counts: &RunNodeCounts, ending: RunEnding) -> &'static str {
    let good: bool = counts.succeeded > 0;
    let bad: bool = counts.failed + counts.skipped + counts.not_started > 0;
    match (ending, bad, good) {
        (RunEnding::Interrupted, _, _) => CANCELLED_STATUS,
        (RunEnding::Raised, _, _) | (RunEnding::Completed, true, false) => FAILED_STATUS,
        (RunEnding::Completed, true, true) => PARTIAL_STATUS,
        (RunEnding::Completed, false, _) => SUCCEEDED_STATUS,
    }
}
