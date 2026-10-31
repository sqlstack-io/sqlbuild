//! Tick history contains immutable evaluations, including covered schedule times.

use uuid::Uuid;

use crate::scheduler::models::TickRecord;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn list_ticks(
    store: &ControlStore,
    trigger_id: Uuid,
    limit: u32,
) -> Result<Vec<TickRecord>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(transaction,
        "SELECT tick_id, trigger_id, scheduled_for, evaluated_at, epoch, outcome, run_id, covered_by_run_id, reason \
         FROM sqb_ticks WHERE trigger_id=$1 ORDER BY scheduled_for DESC LIMIT $2",
        vec![Parameter::Id(trigger_id), Parameter::Integer(i64::from(limit.min(1000)))]).await?;
    let ticks = rows
        .into_iter()
        .map(decode_row)
        .collect::<Result<Vec<_>, _>>()?;
    commit(transaction).await?;
    Ok(ticks)
}
