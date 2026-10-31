//! A heartbeat replaces one instance's liveness, never appending an execution fact.

use serde_json::json;
use uuid::Uuid;

use crate::platform::models::LocationHealthReport;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn location_heartbeat(
    transaction: ControlTransaction,
    location_id: Uuid,
    report: &LocationHealthReport,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT instance.location_id AS instance_location, instance.health_json AS report \
         FROM sqb_locations location LEFT JOIN sqb_location_instances instance ON instance.instance_id=$1 \
         WHERE location.location_id=$2 AND location.archived_at IS NULL",
        vec![Parameter::Id(report.session_id), Parameter::Id(location_id)]).await?;
    let row = rows
        .first()
        .ok_or_else(|| ControlError::Invalid("active location is required".into()))?;
    if let Some(previous) = row.get("report").filter(|value| !value.is_null()) {
        let previous: LocationHealthReport = serde_json::from_value(previous.clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        if row.get("instance_location") != Some(&json!(location_id))
            || previous.target != report.target
        {
            return Err(ControlError::Invalid(
                "location instance belongs to another scope".into(),
            ));
        }
        if previous == *report {
            return Ok((transaction, false));
        }
        if previous.event_id == report.event_id {
            return Err(ControlError::Invalid(
                "location health event ID already has different content".into(),
            ));
        }
    }
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_location_instances (instance_id, location_id, target, connected_at, last_seen_at, health_json) \
         VALUES ($1, $2, $3, $4, $4, $5) ON CONFLICT (instance_id) DO UPDATE SET last_seen_at=$4, health_json=$5",
        vec![Parameter::Id(report.session_id), Parameter::Id(location_id), Parameter::Text(report.target.clone()),
             Parameter::Time(now), Parameter::Json(json!(report))]).await?;
    Ok((transaction, true))
}
