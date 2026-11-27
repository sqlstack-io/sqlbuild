//! A location's health report as a durable event, and the change that reaches the pages people watch.

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::models::{Change, ControlEvent, ControlTransaction};

/// Append the report's event, refusing one that conflicts with a durable fact, and announce what it changed.
pub(crate) async fn append_health(
    transaction: ControlTransaction,
    location_id: Uuid,
    (report, event): (&LocationHealthReport, ControlEvent<'static>),
) -> Result<ControlTransaction, ControlError> {
    let (mut transaction, outcome) = append_control_event(transaction, event).await?;
    if outcome.conflict.is_some() {
        return Err(ControlError::Invalid(
            "location health conflicts with a durable fact".into(),
        ));
    }
    transaction.changes.insert(changed(location_id, report));
    Ok(transaction)
}

/// What a report changes: its warehouse sync, or the rest of its location target's health.
fn changed(location_id: Uuid, report: &LocationHealthReport) -> Change {
    let target: String = report.target.clone();
    match report.health {
        LocationHealth::WarehouseSync { .. } => Change::WarehouseSync(location_id, target),
        _ => Change::LocationHealth(location_id, target),
    }
}

/// The durable event of a report: one ID per location and report, so a retried report is the same event.
pub(crate) fn health_event(
    location_id: Uuid,
    report: &LocationHealthReport,
    now: DateTime<Utc>,
) -> ControlEvent<'static> {
    ControlEvent {
        event_id: Uuid::new_v5(&location_id, report.event_id.as_bytes()),
        invocation_id: format!("location/{location_id}/{}", now.date_naive()),
        event_type: "location_health",
        producer: "location",
        occurred_at: now,
        run_id: None,
        payload: json!({"location_id": location_id, "report": report}),
    }
}
