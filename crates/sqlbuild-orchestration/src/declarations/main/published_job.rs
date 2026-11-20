//! The job a target currently publishes under a name.

use uuid::Uuid;

use crate::declarations::models::StoredJob;
use crate::snapshots::main::reads::decode_published_row::decode_published_row;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// `None` when the target publishes no job or group by that name; a stored job that no longer decodes is `ControlError::UnreadableSnapshot` for its snapshot.
pub(crate) async fn published_job(
    transaction: ControlTransaction,
    (location_id, target, job_key): (Uuid, &str, &str),
) -> Result<(ControlTransaction, Option<StoredJob>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT job_key, kind, snapshot_id, definition_json FROM sqb_jobs \
         WHERE location_id=$1 AND target=$2 AND job_key=$3",
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::Text(job_key.into()),
        ],
    )
    .await?;
    let stored = rows
        .into_iter()
        .next()
        .map(decode_published_row::<StoredJob>)
        .transpose()?;
    Ok((transaction, stored))
}
