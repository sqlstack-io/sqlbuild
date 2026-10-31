//! Runs resolve immutable snapshot documents, never a later deployment's job projections.

use uuid::Uuid;

use crate::declarations::_helpers::documents::decode_definitions;
use crate::declarations::models::Definitions;
use crate::snapshots::main::read_snapshot_documents::read_snapshot_documents;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn frozen_definitions(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
) -> Result<(ControlTransaction, Definitions), ControlError> {
    let (transaction, documents) = read_snapshot_documents(
        transaction,
        snapshot_id,
        Some(&["jobs", "groups", "triggers"]),
    )
    .await?;
    Ok((transaction, decode_definitions(&documents)?))
}
