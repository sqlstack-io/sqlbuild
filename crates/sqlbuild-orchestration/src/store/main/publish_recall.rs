//! Worker publication is atomic; scheduler writes use the same append inside their tick transaction.

use sqlbuild_recall::records::models::RecallRow;

use crate::store::errors::ControlError;
use crate::store::main::append_recall::append_recall;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, RecallAppendOutcome, TransactionMode};

/// Preserve the first copy of each key and record differing retries as Recall conflicts.
pub async fn publish_recall(
    store: &ControlStore,
    rows: &[RecallRow],
) -> Result<Vec<RecallAppendOutcome>, ControlError> {
    let mut transaction = begin(store, TransactionMode::Write).await?;
    let mut outcomes = Vec::with_capacity(rows.len());
    for row in rows {
        let outcome;
        (transaction, outcome) = append_recall(transaction, row).await?;
        outcomes.push(outcome);
    }
    commit(transaction).await?;
    Ok(outcomes)
}
