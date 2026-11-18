//! A completed build's node counts, as its footer counts them.

use sqlbuild_host::models::BuildExecutionResult;
use sqlbuild_state::runs::models::RunNodeCounts;

/// A completed build's node counts, as its footer counts them; every planned node reports a status.
pub fn build_node_counts<V>(result: &BuildExecutionResult<V>) -> RunNodeCounts {
    RunNodeCounts {
        succeeded: result.success_count,
        warned: result.warning_count,
        failed: result.failure_count,
        skipped: result.skipped_count,
        not_started: 0,
    }
}
