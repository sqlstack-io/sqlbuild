//! The internal history row limit.

use crate::cost_command::constants::DEFAULT_HISTORY_LIMIT;

/// None for `--no-limit`, else the requested limit or the default.
#[must_use]
pub(crate) fn cost_history_limit(limit: Option<i64>, no_limit: bool) -> Option<i64> {
    if no_limit {
        return None;
    }
    Some(limit.unwrap_or(DEFAULT_HISTORY_LIMIT))
}
