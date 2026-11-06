//! Read a trigger's operator changes from the security audit in committed order.

use uuid::Uuid;

use crate::declarations::models::TriggerChangeRecord;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn list_trigger_events(
    store: &ControlStore,
    trigger_id: Uuid,
    limit: u32,
) -> Result<Vec<TriggerChangeRecord>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT scope_json, occurred_at FROM sqb_security_audit WHERE action='trigger.changed' \
         AND scope_json ->> 'trigger_id'=$1 ORDER BY occurred_at, event_id LIMIT $2",
        vec![
            Parameter::Text(trigger_id.to_string()),
            Parameter::Integer(i64::from(limit.min(1000))),
        ],
    )
    .await?;
    let events = rows
        .into_iter()
        .map(|row| {
            serde_json::from_value(serde_json::json!({
                "request_json": row["scope_json"]["request"],
                "snapshot_id": row["scope_json"]["snapshot_id"],
                "occurred_at": row["occurred_at"],
            }))
            .map_err(|error| ControlError::Schema(format!("trigger change: {error}")))
        })
        .collect::<Result<Vec<TriggerChangeRecord>, _>>()?;
    commit(transaction).await?;
    Ok(events)
}
