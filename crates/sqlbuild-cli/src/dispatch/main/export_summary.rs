//! `export_summary`: a command's lifecycle export accounting at exit, as its warning reads it.

use sqlbuild_core::event_export::models::ExportSnapshot;

use crate::dispatch::models::{ExportSummary, ExporterLoss};

/// The accounting of `snapshot`, with each sink provider whose teardown raised.
#[must_use]
pub fn export_summary(snapshot: &ExportSnapshot, teardown_failures: Vec<String>) -> ExportSummary {
    ExportSummary {
        accepted: snapshot.aggregate.accepted,
        dropped: snapshot.aggregate.dropped,
        failed: snapshot.aggregate.failed,
        per_exporter: snapshot
            .per_exporter
            .iter()
            .map(|(name, counts)| ExporterLoss {
                exporter_name: name.clone(),
                dropped: counts.dropped,
                failed: counts.failed,
            })
            .collect(),
        teardown_failures,
    }
}
