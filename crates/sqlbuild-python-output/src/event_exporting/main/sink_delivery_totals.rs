//! `sink_delivery_totals`: what each sink of the active exporter scope has delivered so far.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_orchestration::platform::models::SinkDeliveryTotals;
use sqlbuild_orchestration::runs::models::SinkDelivery;
use sqlbuild_python_base::event_exporting::main::current_scope::current_scope;
use sqlbuild_python_base::event_exporting::models::{
    EventExportSummary, EventExporterCommandScope,
};

use crate::event_exporting::models::EventExporterDispatcher;

/// Each sink's totals since the active scope opened; none without a scope.
pub fn sink_delivery_totals(py: Python<'_>) -> PyResult<SinkDeliveryTotals> {
    let scope: Bound<'_, PyAny> = current_scope(py)?;
    let Ok(scope) = scope.cast::<EventExporterCommandScope>() else {
        return Ok(SinkDeliveryTotals::default());
    };
    let dispatcher = scope
        .get()
        .dispatcher(py)
        .into_bound(py)
        .cast_into::<EventExporterDispatcher>()?;
    let summary = dispatcher.get().summary(py)?;
    let mut sinks: Vec<SinkDelivery> = Vec::new();
    for (name, counts) in &summary
        .bind(py)
        .cast::<EventExportSummary>()?
        .get()
        .snapshot
        .per_exporter
    {
        sinks.push(SinkDelivery {
            name: name.clone(),
            accepted: counts.accepted,
            delivered: counts.delivered,
            dropped: counts.dropped,
            failed: counts.failed,
        });
    }
    Ok(SinkDeliveryTotals { sinks })
}
