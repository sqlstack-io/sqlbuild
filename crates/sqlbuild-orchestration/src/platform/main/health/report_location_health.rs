//! Append scoped location observations without turning health into mutable lifecycle state.

use serde_json::json;
use uuid::Uuid;

use crate::platform::_helpers::location::location_heartbeat::location_heartbeat;
use crate::platform::_helpers::location::published_generation::require_published_generation;
use crate::platform::_helpers::location::reported_generation::reported_generation;
use crate::platform::_helpers::location::rule_findings::check_rule_findings;
use crate::platform::models::LocationHealthReport;
use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlStore, Parameter, TransactionMode};

pub async fn report_location_health(
    store: &ControlStore,
    location_id: Uuid,
    report: &LocationHealthReport,
) -> Result<bool, ControlError> {
    let generation = reported_generation(report)?;
    check_rule_findings(report)?;
    let transaction = begin(store, TransactionMode::Write).await?;
    if generation.is_none() {
        let (transaction, applied) = location_heartbeat(transaction, location_id, report).await?;
        commit(transaction).await?;
        return Ok(applied);
    }
    let invocation_id = format!("location/{location_id}/{}", transaction.now.date_naive());
    let event_id = Uuid::new_v5(&location_id, report.event_id.as_bytes());
    let payload = json!({"location_id": location_id, "report": report});
    let (transaction, previous) = query(
        transaction,
        "SELECT payload FROM recall_events WHERE event_id=$1 LIMIT 1",
        vec![Parameter::Text(event_id.to_string())],
    )
    .await?;
    if let Some(previous) = previous.first() {
        if previous.get("payload") != Some(&payload) {
            return Err(ControlError::Invalid(
                "location health event ID already has different content".into(),
            ));
        }
        commit(transaction).await?;
        return Ok(false);
    }
    let (transaction, location) = query(
        transaction,
        "SELECT location_id FROM sqb_locations WHERE location_id=$1 AND archived_at IS NULL",
        vec![Parameter::Id(location_id)],
    )
    .await?;
    if location.is_empty() {
        return Err(ControlError::Invalid("active location is required".into()));
    }
    let transaction = require_published_generation(transaction, location_id, report).await?;
    let occurred_at = transaction.now;
    let (transaction, outcome) = append_control_event(
        transaction,
        ControlEvent {
            event_id,
            invocation_id,
            event_type: "location_health",
            producer: "location",
            occurred_at,
            run_id: None,
            payload,
        },
    )
    .await?;
    if outcome.conflict.is_some() {
        return Err(ControlError::Invalid(
            "location health conflicts with a durable fact".into(),
        ));
    }
    commit(transaction).await?;
    Ok(true)
}
