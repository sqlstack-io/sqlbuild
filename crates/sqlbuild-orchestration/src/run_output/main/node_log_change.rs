//! One `ctx.log` line as the run change a worker reports.

use crate::run_output::main::bounded_log_message::bounded_log_message;
use crate::runs::models::{NodeLogLevel, RunChange};

/// The `index`th line `node_key` logged, with the event key it reports under; the same line always gets the same key.
#[must_use]
pub fn node_log_change(
    node_key: &str,
    index: usize,
    (level, message): (NodeLogLevel, &str),
) -> (String, RunChange) {
    (
        format!("node-log/{node_key}/{index}"),
        RunChange::NodeLog {
            node_key: node_key.to_owned(),
            level,
            message: bounded_log_message(message),
        },
    )
}
