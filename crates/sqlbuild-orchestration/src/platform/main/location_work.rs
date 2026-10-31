//! Read one request and its durable result without persisting a derived pending/expired status.

use uuid::Uuid;

use crate::platform::_helpers::location_work::read_work;
use crate::platform::models::LocationWorkRecord;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

pub async fn location_work(
    store: &ControlStore,
    request_id: Uuid,
) -> Result<Option<LocationWorkRecord>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, record) = read_work(transaction, request_id).await?;
    commit(transaction).await?;
    Ok(record.map(|stored| stored.record))
}
