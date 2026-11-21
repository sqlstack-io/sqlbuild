//! Runs resolve immutable snapshot documents, never a later deployment's job projections; an unreadable snapshot leaves the caller's transaction usable.

use uuid::Uuid;

use crate::declarations::_helpers::documents::decode_definitions;
use crate::declarations::models::Definitions;
use crate::snapshots::main::reads::read_snapshot_documents::read_snapshot_documents;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// The declarations of `snapshot_id`; a document that no longer decodes is `ControlError::UnreadableSnapshot` inside the result, so the caller can still fail only its own unit in the same transaction.
pub(crate) async fn frozen_definitions(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
) -> Result<(ControlTransaction, Result<Definitions, ControlError>), ControlError> {
    let (transaction, documents) = read_snapshot_documents(
        transaction,
        snapshot_id,
        Some(&["jobs", "groups", "triggers"]),
    )
    .await?;
    let definitions =
        decode_definitions(&documents).map_err(|detail| ControlError::UnreadableSnapshot {
            snapshot_id,
            run_id: None,
            detail,
        });
    Ok((transaction, definitions))
}
