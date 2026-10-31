//! Exact-byte Recall output is namespaced by the authenticated physical claim, never a body ID.

use sqlbuild_recall::records::models::OutputChunk;
use uuid::Uuid;

use crate::runs::_helpers::lifecycle::output::{claim_envelope, output_rows};
use crate::runs::models::OutputPublication;
use crate::store::errors::ControlError;
use crate::store::main::append_recall::append_recall;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

/// The active-claim check and the complete batch commit share a serialized write transaction.
pub async fn publish_run_output(
    store: &ControlStore,
    (run_id, claim_id): (Uuid, Uuid),
    chunks: &[OutputChunk],
) -> Result<OutputPublication, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (mut transaction, envelope) = claim_envelope(transaction, (run_id, claim_id)).await?;
    let mut published = OutputPublication::default();
    for row in output_rows(&envelope, chunks)? {
        let outcome;
        (transaction, outcome) = append_recall(transaction, &row).await?;
        if outcome.inserted {
            published.inserted += 1;
        } else if outcome.conflict.is_some() {
            published.conflicts += 1;
        } else {
            published.duplicates += 1;
        }
    }
    commit(transaction).await?;
    Ok(published)
}
