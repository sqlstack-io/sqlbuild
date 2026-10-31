//! A worker's complete Recall batch and active-claim check share one transaction.

use sqlbuild_recall::store::models::InsertOutcome;
use uuid::Uuid;

use crate::runs::_helpers::lifecycle::output::claim_envelope;
use crate::runs::_helpers::lifecycle::recall::{append_records, claim_record};
use crate::runs::constants::{RECALL_BATCH_ROWS, RECALL_INVOCATION_ID_BYTES};
use crate::runs::models::RecallBatch;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

pub async fn publish_run_recall(
    store: &ControlStore,
    identity: (Uuid, Uuid),
    batch: &RecallBatch,
) -> Result<InsertOutcome, ControlError> {
    if batch.invocation_id.trim().is_empty()
        || batch.invocation_id.len() > RECALL_INVOCATION_ID_BYTES
        || batch.records.len() > RECALL_BATCH_ROWS
    {
        return Err(ControlError::Invalid(
            "invalid Recall invocation or batch size".into(),
        ));
    }
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, mut envelope) = claim_envelope(transaction, identity).await?;
    envelope.invocation_id = format!("{}/{}", envelope.invocation_id, batch.invocation_id);
    let rows = batch
        .records
        .iter()
        .map(|record| claim_record(&envelope, record))
        .collect::<Result<Vec<_>, _>>()?;
    let (transaction, result) = append_records(transaction, &envelope, &rows).await?;
    commit(transaction).await?;
    Ok(result)
}
