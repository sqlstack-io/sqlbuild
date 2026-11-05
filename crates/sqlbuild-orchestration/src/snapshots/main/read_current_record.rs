//! Read publication identity inside the caller's existing authorization transaction.

use uuid::Uuid;

use crate::snapshots::models::SnapshotRecord;
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_current_record(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Option<SnapshotRecord>), ControlError> {
    let (transaction, records) = query_records(transaction,
        "SELECT s.snapshot_id, s.location_id, s.target, s.generation, s.manifest_digest, s.published_at \
         FROM sqb_current_snapshots c JOIN sqb_snapshots s ON s.snapshot_id=c.snapshot_id WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
    Ok((transaction, records.into_iter().next()))
}
