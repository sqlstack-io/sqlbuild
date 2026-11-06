//! Tick history contains immutable evaluations, including covered schedule times.

use uuid::Uuid;

use crate::scheduler::_helpers::reads::ticks_in;
use crate::scheduler::models::TickRecord;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn list_ticks(
    store: &ControlStore,
    trigger_id: Uuid,
    limit: u32,
) -> Result<Vec<TickRecord>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, ticks) = ticks_in(transaction, trigger_id, limit).await?;
    commit(transaction).await?;
    Ok(ticks)
}
