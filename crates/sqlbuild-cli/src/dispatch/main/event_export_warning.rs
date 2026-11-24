//! The stderr warning for lifecycle events dropped or failed at command exit.

use sqlbuild_core::event_export::constants::DEFAULT_EVENT_EXPORT_INVOCATION_TIMEOUT_SECONDS;

use crate::dispatch::constants::SHUTDOWN_TIMEOUT_SETTING;
use crate::dispatch::models::{ExportSummary, ExporterLoss};

/// One warning line when lifecycle events were dropped or failed or a sink provider's teardown raised, else `None`.
#[must_use]
pub fn event_export_warning(summary: &ExportSummary, command_succeeded: bool) -> Option<String> {
    let lost: bool = summary.dropped > 0 || summary.failed > 0;
    if !lost && summary.teardown_failures.is_empty() {
        return None;
    }
    let affected: Vec<&ExporterLoss> = summary
        .per_exporter
        .iter()
        .filter(|exporter| exporter.dropped > 0 || exporter.failed > 0)
        .collect();
    let names: String = affected
        .iter()
        .map(|exporter| format!("'{}'", exporter.exporter_name))
        .collect::<Vec<String>>()
        .join(", ");
    let sink_label: &str = if affected.len() == 1 { "sink" } else { "sinks" };
    let unit: &str = if summary.per_exporter.len() == 1 {
        "events"
    } else {
        "event deliveries"
    };
    let prefix: &str = if command_succeeded {
        "Warning: command completed successfully, but lifecycle event export was incomplete"
    } else {
        "Warning: lifecycle event export was incomplete"
    };
    let mut parts: Vec<String> = Vec::new();
    if lost {
        parts.push(format!(
            "{} of {} {unit} dropped, {} failed ({sink_label} {names}). \
             Check sink health: a sink call slower than {DEFAULT_EVENT_EXPORT_INVOCATION_TIMEOUT_SECONDS}s \
             drops that sink's events until it returns; events still queued at exit need a longer \
             {SHUTDOWN_TIMEOUT_SETTING}.",
            summary.dropped, summary.accepted, summary.failed
        ));
    }
    if !summary.teardown_failures.is_empty() {
        parts.push(format!(
            "sink provider teardown failed ({}); events the provider still held may not have \
             been delivered.",
            summary.teardown_failures.join(", ")
        ));
    }
    Some(format!("{prefix}: {}", parts.join(" ")))
}
