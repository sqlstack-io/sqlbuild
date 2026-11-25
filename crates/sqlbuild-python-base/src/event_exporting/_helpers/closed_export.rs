//! The newest command scope this thread closed, kept for the worker that reports the run.

use std::cell::RefCell;

use pyo3::prelude::{Bound, PyAny};

use crate::event_exporting::models::{ClosedExport, EventExportSummary};

thread_local! {
    pub(crate) static CLOSED_EXPORT: RefCell<Option<ClosedExport>> = const { RefCell::new(None) };
}

/// Keep `summary`'s per-sink counts and the `teardown_failures` for `take_closed_export`.
pub(crate) fn record_closed_export(summary: &Bound<'_, PyAny>, teardown_failures: Vec<String>) {
    let per_exporter = summary
        .cast::<EventExportSummary>()
        .map(|summary| summary.get().snapshot.per_exporter.clone())
        .unwrap_or_default();
    CLOSED_EXPORT.with(|closed| {
        *closed.borrow_mut() = Some(ClosedExport {
            per_exporter,
            teardown_failures,
        });
    });
}
