//! Pin new location work to the current publication and audit the request in the caller's transaction.

use chrono::Duration;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::platform::models::LocationWorkRequest;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// The caller has already authorised the request and ruled out a retry of the same request ID.
pub(crate) async fn enqueue_location_work(
    transaction: ControlTransaction,
    context: &AuditContext,
    request: &LocationWorkRequest,
    lifetime: Duration,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT s.snapshot_id, s.generation FROM sqb_current_snapshots c \
         JOIN sqb_snapshots s ON s.snapshot_id=c.snapshot_id JOIN sqb_locations l ON l.location_id=c.location_id \
         WHERE c.location_id=$1 AND c.target=$2 AND l.archived_at IS NULL",
        vec![Parameter::Id(request.location_id), Parameter::Text(request.target.clone())]).await?;
    let snapshot = rows
        .first()
        .ok_or_else(|| ControlError::Invalid("location work requires a published target".into()))?;
    let snapshot_id = serde_json::from_value(snapshot["snapshot_id"].clone())
        .map_err(|error| ControlError::Invalid(format!("invalid snapshot identity: {error}")))?;
    let generation = snapshot["generation"]
        .as_i64()
        .ok_or_else(|| ControlError::Invalid("invalid snapshot generation".into()))?;
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_location_work \
         (request_id,location_id,target,snapshot_id,generation,request_json,requested_by,requested_at,expires_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        vec![Parameter::Id(request.request_id), Parameter::Id(request.location_id), Parameter::Text(request.target.clone()),
            Parameter::Id(snapshot_id), Parameter::Integer(generation), Parameter::Json(json!(request)),
            Parameter::Text(context.actor.clone()), Parameter::Time(now), Parameter::Time(now + lifetime)]).await?;
    append_audit(
        transaction,
        context,
        (
            Uuid::new_v5(&request.request_id, b"location-work-requested"),
            "location.work_requested",
        ),
        (
            json!({"location_id": request.location_id, "target": request.target}),
            json!(null),
            json!(request),
        ),
    )
    .await
}
