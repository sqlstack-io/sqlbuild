//! The `ctx.log` lines a finished node reports, the note a worker leaves when run output was lost, and its sinks' deliveries.

use sqlbuild_host::models::{LifeCycleEvent, LifeCycleEventKind, LogLevel};
use sqlbuild_orchestration::run_output::constants::{
    NODE_LOG_MESSAGE_BYTES, NODE_LOGS_PER_NODE, RUN_OUTPUT_NODE,
};
use sqlbuild_orchestration::run_output::models::UploadSummary;
use sqlbuild_orchestration::runs::models::{NodeLogLevel, RunChange, SinkDelivery};
use sqlbuild_python_base::event_exporting::models::ClosedExport;

const ELLIPSIS: char = '\u{2026}';

/// Each of the node's first `ctx.log` lines with the event key it reports under; lines the executor logs itself carry no level and stay out.
pub(crate) fn node_log_changes(
    node_key: &str,
    events: &[LifeCycleEvent],
) -> Vec<(String, RunChange)> {
    events
        .iter()
        .filter(|event| matches!(event.kind, LifeCycleEventKind::Log) && event.level.is_some())
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

/// What was lost when some of a run's output could not be sent, and the warning reporting it under the run; `None` when nothing was lost.
pub(crate) fn upload_loss(summary: &UploadSummary) -> Option<(String, RunChange)> {
    let failure: String = summary
        .failure
        .clone()
        .map_or_else(String::new, |failure| format!(": {failure}"));
    (summary.dropped_bytes > 0 || summary.failure.is_some()).then(|| {
        let message: String = bounded(&format!(
            "{} bytes of run output were not stored{failure}",
            summary.dropped_bytes
        ));
        (
            message.clone(),
            RunChange::NodeLog {
                node_key: RUN_OUTPUT_NODE.to_owned(),
                level: NodeLogLevel::Warning,
                message,
            },
        )
    })
}

/// `message` without NUL, which PostgreSQL refuses in text and JSON, cut to its limit at a character boundary, ending in an ellipsis when cut.
fn bounded(message: &str) -> String {
    let message: String = message.replace('\0', "");
    if message.len() <= NODE_LOG_MESSAGE_BYTES {
        return message;
    }
    let room: usize = NODE_LOG_MESSAGE_BYTES - ELLIPSIS.len_utf8();
    let end: usize = (0..=room)
        .rev()
        .find(|index| message.is_char_boundary(*index))
        .unwrap_or(0);
    format!("{}{ELLIPSIS}", &message[..end])
}

/// What each lifecycle event sink of the worker's command delivered; `None` when it had no sinks.
pub(crate) fn sink_deliveries(closed: ClosedExport) -> Option<RunChange> {
    (!closed.per_exporter.is_empty() || !closed.teardown_failures.is_empty()).then(|| {
        RunChange::SinkDeliveries {
            sinks: closed
                .per_exporter
                .into_iter()
                .map(|(name, counts)| SinkDelivery {
                    name,
                    accepted: counts.accepted,
                    delivered: counts.delivered,
                    dropped: counts.dropped,
                    failed: counts.failed,
                })
                .collect(),
            teardown_failures: closed.teardown_failures,
        }
    })
}
