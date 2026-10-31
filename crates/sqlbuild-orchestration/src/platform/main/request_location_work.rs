//! Only trusted store owners enqueue named work; retries preserve the original generation/deadline.

use chrono::Duration;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::platform::_helpers::location_work::read_work;
use crate::platform::models::LocationWorkRequest;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn request_location_work(
    store: &ControlStore,
    context: &AuditContext,
    request: &LocationWorkRequest,
) -> Result<bool, ControlError> {
    if request.target.trim().is_empty()
        || request.reason.trim().is_empty()
        || context.actor.trim().is_empty()
    {
        return Err(ControlError::Invalid(
            "location work needs a target, reason and trusted actor".into(),
        ));
    }
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, previous) = read_work(transaction, request.request_id).await?;
    if let Some(previous) = previous {
        if previous.record.request_json != *request || previous.record.requested_by != context.actor
        {
            return Err(ControlError::Invalid(
                "location work ID already has different content or actor".into(),
            ));
        }
        commit(transaction).await?;
        return Ok(false);
    }
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
            Parameter::Text(context.actor.clone()), Parameter::Time(now), Parameter::Time(now + Duration::minutes(5))]).await?;
    commit(
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
        .await?,
    )
    .await?;
    Ok(true)
}
