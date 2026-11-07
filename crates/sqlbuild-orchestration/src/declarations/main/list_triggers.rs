//! Current declarations and runtime state are read in one consistent transaction.

use uuid::Uuid;

use crate::declarations::_helpers::reads::triggers_in;
use crate::declarations::models::TriggerRecord;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn list_triggers(
    store: &ControlStore,
    location_id: Uuid,
    target: &str,
) -> Result<Vec<TriggerRecord>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, triggers) = triggers_in(transaction, (location_id, target)).await?;
    commit(transaction).await?;
    Ok(triggers)
}
