//! Read immutable operator changes in committed order.

use uuid::Uuid;

use crate::declarations::models::TriggerChangeRecord;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn list_trigger_events(
    store: &ControlStore,
    trigger_id: Uuid,
    limit: u32,
) -> Result<Vec<TriggerChangeRecord>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(transaction,
        "SELECT request_json, occurred_at, snapshot_id FROM sqb_trigger_state_events WHERE trigger_id=$1 ORDER BY occurred_at, event_id LIMIT $2",
        vec![Parameter::Id(trigger_id), Parameter::Integer(i64::from(limit.min(1000)))]).await?;
    let events = rows
        .into_iter()
        .map(decode_row)
        .collect::<Result<Vec<_>, _>>()?;
    commit(transaction).await?;
    Ok(events)
}
