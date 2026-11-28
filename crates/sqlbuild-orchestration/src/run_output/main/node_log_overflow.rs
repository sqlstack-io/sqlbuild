//! The warning line that says how many of a node's `ctx.log` lines were not kept.

use crate::run_output::constants::{NODE_LOG_OVERFLOW_MESSAGE, NODE_LOGS_PER_NODE};
use crate::run_output::main::node_log_change::node_log_change;
use crate::runs::models::{NodeLogLevel, RunChange};

/// The line after a node's last kept one, when it logged `logged` lines; `None` when all were kept.
#[must_use]
pub fn node_log_overflow(node_key: &str, logged: usize) -> Option<(String, RunChange)> {
    let dropped: usize = logged
        .checked_sub(NODE_LOGS_PER_NODE)
        .filter(|count| *count > 0)?;
    let message: String = NODE_LOG_OVERFLOW_MESSAGE
        .replace("{count}", &dropped.to_string())
        .replace("{limit}", &NODE_LOGS_PER_NODE.to_string());
    Some(node_log_change(
        node_key,
        NODE_LOGS_PER_NODE,
        (NodeLogLevel::Warning, &message),
    ))
}
