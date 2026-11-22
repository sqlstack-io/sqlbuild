//! Discard the running command's `--json` report, so a worker's printed output is only human output.

use crate::result_output::_helpers::results::discarded_reports::REPORTS_DISCARDED;
use crate::result_output::models::DiscardedReportsScope;

/// Discard every `--json` report written to stdout on this thread until the scope is dropped; nothing reads a worker's report, so it is neither printed nor stored.
#[must_use]
pub fn discard_reports() -> DiscardedReportsScope {
    DiscardedReportsScope {
        previous: REPORTS_DISCARDED.replace(true),
    }
}
