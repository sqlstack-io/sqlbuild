//! Stable cursor paging follows serialized committed appends, not producer timestamps.

use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::types::Row;

/// Page events in storage order; equal producer timestamps and late arrivals remain visible.
pub async fn read_recall_events(
    store: &ControlStore,
    after: i64,
    limit: u32,
) -> Result<Vec<Row>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT * FROM recall_events WHERE storage_id > $1 ORDER BY storage_id LIMIT $2",
        vec![
            Parameter::Integer(after),
            Parameter::Integer(i64::from(limit.min(1000))),
        ],
    )
    .await?;
    commit(transaction).await?;
    Ok(rows)
}
