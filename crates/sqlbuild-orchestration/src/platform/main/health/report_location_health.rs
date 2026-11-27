//! Append scoped location observations without turning health into mutable lifecycle state.

use uuid::Uuid;

use crate::platform::_helpers::location::health_events::{append_health, health_event};
use crate::platform::_helpers::location::location_heartbeat::location_heartbeat;
use crate::platform::_helpers::location::published_generation::require_published_generation;
use crate::platform::_helpers::location::reported_generation::reported_generation;
use crate::platform::_helpers::location::rule_findings::check_rule_findings;
use crate::platform::_helpers::plans::load_failure::fail_plans_on_failed_load;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn report_location_health(
    store: &ControlStore,
    location_id: Uuid,
    report: &LocationHealthReport,
) -> Result<bool, ControlError> {
    reported_generation(report)?;
    check_rule_findings(report)?;
    let transaction = begin(store, TransactionMode::Write).await?;
    if matches!(report.health, LocationHealth::Heartbeat {}) {
        let (transaction, applied) = location_heartbeat(transaction, location_id, report).await?;
        commit(transaction).await?;
        return Ok(applied);
    }
    let event = health_event(location_id, report, transaction.now);
    let (transaction, previous) = query(
        transaction,
        "SELECT payload FROM recall_events WHERE event_id=$1 LIMIT 1",
        vec![Parameter::Text(event.event_id.to_string())],
    )
    .await?;
    if let Some(previous) = previous.first() {
        if previous.get("payload") != Some(&event.payload) {
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
    let transaction = append_health(transaction, location_id, (report, event)).await?;
    let transaction = fail_plans_on_failed_load(transaction, location_id, report).await?;
    commit(transaction).await?;
    Ok(true)
}
