//! Record immutable semantic progress in the transition owner's existing transaction.

use uuid::Uuid;

use crate::declarations::models::TriggerProjection;
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn record_trigger_progress(
    transaction: ControlTransaction,
    trigger_id: Uuid,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT t.*,s.status,s.status_override,s.started_at,s.cursor_json,s.last_tick_for,\
         s.next_due_at,s.consecutive_failures,s.updated_at FROM sqb_triggers t \
         JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id WHERE t.trigger_id=$1",
        vec![Parameter::Id(trigger_id)],
    )
    .await?;
    let projection: TriggerProjection = decode_row(rows.into_iter().next().ok_or_else(|| {
        ControlError::Schema("trigger transition has no complete projection".into())
    })?)?;
    let body = serde_json::to_value(&projection)
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (transaction, previous) = query(transaction,
        "SELECT schema_version,projection_json FROM sqb_trigger_progress WHERE trigger_id=$1 ORDER BY storage_id DESC LIMIT 1",
        vec![Parameter::Id(trigger_id)]).await?;
    if previous.first().is_some_and(|row| {
        row["schema_version"].as_i64() == Some(1) && row["projection_json"] == body
    }) {
        return Ok(transaction);
    }
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_trigger_progress (storage_id,trigger_id,snapshot_id,schema_version,projection_json) \
         SELECT COALESCE(MAX(storage_id),0)+1,$1,$2,1,$3 FROM sqb_trigger_progress",
        vec![Parameter::Id(trigger_id), Parameter::Id(projection.snapshot_id), Parameter::Json(body)]).await?;
    Ok(transaction)
}
