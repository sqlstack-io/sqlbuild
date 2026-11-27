//! The `ctx.log` lines a finished node reports, the note a worker leaves when run output was lost, and its sinks' deliveries.

use sqlbuild_host::models::{LifeCycleEvent, LifeCycleEventKind, LogLevel};
use sqlbuild_orchestration::run_output::constants::{NODE_LOGS_PER_NODE, RUN_OUTPUT_NODE};
use sqlbuild_orchestration::run_output::main::bounded_log_message::bounded_log_message;
use sqlbuild_orchestration::run_output::main::node_log_change::node_log_change;
use sqlbuild_orchestration::run_output::main::node_log_overflow::node_log_overflow;
use sqlbuild_orchestration::run_output::models::UploadSummary;
use sqlbuild_orchestration::runs::models::{NodeLogLevel, RunChange, SinkDelivery};
use sqlbuild_python_base::event_exporting::models::ClosedExport;

/// The level a `ctx.log` line reports at.
pub(crate) fn node_log_level(level: LogLevel) -> NodeLogLevel {
    match level {
        LogLevel::Warning => NodeLogLevel::Warning,
        LogLevel::Error => NodeLogLevel::Error,
        LogLevel::Info => NodeLogLevel::Info,
    }
}

/// Each of the node's first `ctx.log` lines with the event key it reports under, then a warning saying how many more were not kept; lines the executor logs itself carry no level and stay out.
pub(crate) fn node_log_changes(
    node_key: &str,
    events: &[LifeCycleEvent],
) -> Vec<(String, RunChange)> {
    let logged: Vec<(NodeLogLevel, &str)> = events
        .iter()
        .filter(|event| matches!(event.kind, LifeCycleEventKind::Log))
        .filter_map(|event| Some((node_log_level(event.level?), event.content.as_str())))
        .collect();
    logged
        .iter()
        .take(NODE_LOGS_PER_NODE)
        .enumerate()
        .map(|(index, line)| node_log_change(node_key, index, *line))
        .chain(node_log_overflow(node_key, logged.len()))
        .collect()
}

/// What was lost when some of a run's output could not be sent, and the warning reporting it under the run; `None` when nothing was lost.
pub(crate) fn upload_loss(summary: &UploadSummary) -> Option<(String, RunChange)> {
    let failure: String = summary
        .failure
        .clone()
        .map_or_else(String::new, |failure| format!(": {failure}"));
    (summary.dropped_bytes > 0 || summary.failure.is_some()).then(|| {
        let message: String = bounded_log_message(&format!(
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
