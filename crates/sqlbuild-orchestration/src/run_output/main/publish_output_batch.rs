//! A worker's batch of output: one zstd chunk in the output store, indexed under the worker's active claim.

use uuid::Uuid;

use crate::run_output::_helpers::store::index::{batch_stored, record_chunk};
use crate::run_output::_helpers::store::objects::{chunk_key, delete_chunks, put_chunk};
use crate::run_output::_helpers::store::prepare::{checked, prepared};
use crate::run_output::models::{OutputBatch, OutputBatchReceipt, OutputStore};
use crate::runs::main::execution::require_active_claim::require_active_claim;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// The chunk is written before its index, so a failed index leaves only an object retention removes; a retried batch is stored once.
pub async fn publish_output_batch(
    store: &ControlStore,
    output: &OutputStore,
    (run_id, claim_id): (Uuid, Uuid),
    batch: OutputBatch,
) -> Result<OutputBatchReceipt, ControlError> {
    checked(&batch)?;
    let transaction = begin(store, TransactionMode::Read).await?;
    let transaction = require_active_claim(transaction, (run_id, claim_id)).await?;
    let (transaction, stored) = batch_stored(transaction, (run_id, claim_id), batch.batch).await?;
    commit(transaction).await?;
    if stored {
        return Ok(OutputBatchReceipt { stored: false });
    }
    let key = chunk_key(output, (run_id, claim_id), batch.batch);
    let object_key: String = key.to_string();
    let (chunk, compressed) =
        tokio::task::spawn_blocking(move || prepared((run_id, claim_id), &batch, object_key))
            .await??;
    put_chunk(output, &key, compressed).await?;
    let half: Option<i64> = output
        .max_bytes_per_run
        .map(|limit| i64::try_from(limit / 2).unwrap_or(i64::MAX));
    let transaction = begin(store, TransactionMode::Write).await?;
    let transaction = require_active_claim(transaction, (run_id, claim_id)).await?;
    let (transaction, stored) = batch_stored(transaction, (run_id, claim_id), chunk.batch).await?;
    if stored {
        commit(transaction).await?;
        return Ok(OutputBatchReceipt { stored: false });
    }
    let (transaction, dropped) = record_chunk(transaction, &chunk, half).await?;
    commit(transaction).await?;
    delete_chunks(output, &dropped).await?;
    Ok(OutputBatchReceipt { stored: true })
}
