//! Restore only resolved job projections from immutable publication documents.

use crate::declarations::_helpers::documents::decode_definitions;
use crate::declarations::_helpers::publication::publish_jobs;
use crate::snapshots::main::read_snapshot_documents::read_snapshot_documents;
use crate::snapshots::models::SnapshotRecord;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn rebuild_jobs(
    transaction: ControlTransaction,
    snapshot: &SnapshotRecord,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, documents) =
        read_snapshot_documents(transaction, snapshot.snapshot_id, Some(&["jobs", "groups"]))
            .await?;
    publish_jobs(transaction, snapshot, &decode_definitions(&documents)?).await
}
