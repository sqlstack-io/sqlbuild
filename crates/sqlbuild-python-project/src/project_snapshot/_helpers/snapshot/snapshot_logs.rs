//! The `ctx.log` lines a finished node reports, and the note a worker leaves when run output was lost.

use sqlbuild_host::models::{LifeCycleEvent, LifeCycleEventKind, LogLevel};
use sqlbuild_orchestration::run_output::constants::{
    NODE_LOG_MESSAGE_BYTES, NODE_LOGS_PER_NODE, RUN_OUTPUT_NODE,
};
use sqlbuild_orchestration::run_output::models::UploadSummary;
use sqlbuild_orchestration::runs::models::{NodeLogLevel, RunChange};

const ELLIPSIS: char = '\u{2026}';

/// Each of the node's first `ctx.log` lines with the event key it reports under.
pub(crate) fn node_log_changes(
    node_key: &str,
    events: &[LifeCycleEvent],
) -> Vec<(String, RunChange)> {
    events
        .iter()
        .filter(|event| matches!(event.kind, LifeCycleEventKind::Log))
        .take(NODE_LOGS_PER_NODE)
        .enumerate()
        .map(|(index, event)| {
            (
                format!("node-log/{node_key}/{index}"),
                RunChange::NodeLog {
                    node_key: node_key.to_owned(),
                    level: match event.level {
                        Some(LogLevel::Warning) => NodeLogLevel::Warning,
                        Some(LogLevel::Error) => NodeLogLevel::Error,
                        Some(LogLevel::Info) | None => NodeLogLevel::Info,
                    },
                    message: bounded(&event.content),
                },
            )
        })
        .collect()
}

/// A warning under the run when some of its output could not be sent; `None` when nothing was lost.
pub(crate) fn upload_loss_change(summary: &UploadSummary) -> Option<RunChange> {
    let failure: String = summary
        .failure
        .clone()
        .map_or_else(String::new, |failure| format!(": {failure}"));
    (summary.dropped_bytes > 0 || summary.failure.is_some()).then(|| RunChange::NodeLog {
        node_key: RUN_OUTPUT_NODE.to_owned(),
        level: NodeLogLevel::Warning,
        message: bounded(&format!(
            "{} bytes of run output were not stored{failure}",
            summary.dropped_bytes
        )),
    })
}

/// `message` cut to its limit at a character boundary, ending in an ellipsis when cut.
fn bounded(message: &str) -> String {
    if message.len() <= NODE_LOG_MESSAGE_BYTES {
        return message.to_owned();
    }
    let room: usize = NODE_LOG_MESSAGE_BYTES - ELLIPSIS.len_utf8();
    let end: usize = (0..=room)
        .rev()
        .find(|index| message.is_char_boundary(*index))
        .unwrap_or(0);
    format!("{}{ELLIPSIS}", &message[..end])
}
