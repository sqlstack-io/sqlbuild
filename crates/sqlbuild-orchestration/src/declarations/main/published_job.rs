//! The job a target currently publishes under a name.

use uuid::Uuid;

use crate::declarations::models::StoredJob;
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

/// `None` when the target publishes no job or group by that name.
pub(crate) async fn published_job(
    transaction: ControlTransaction,
    (location_id, target, job_key): (Uuid, &str, &str),
) -> Result<(ControlTransaction, Option<StoredJob>), ControlError> {
    let (transaction, stored): (_, Vec<StoredJob>) = query_records(
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
    Ok((transaction, stored.into_iter().next()))
}
