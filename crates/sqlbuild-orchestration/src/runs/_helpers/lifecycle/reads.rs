//! Bounded metadata projection; it never includes SQL, sampled rows or captured output.

use uuid::Uuid;

use crate::runs::models::RunRecord;
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn list_runs_in(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    limit: u32,
) -> Result<(ControlTransaction, Vec<RunRecord>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT run_id, location_id, target, snapshot_id, job_key, status, queued_at, launched_at, started_at, ended_at, selection_json, node_counts_json, last_event_storage_id \
         FROM sqb_runs WHERE location_id=$1 AND target=$2 ORDER BY queued_at DESC, run_id DESC LIMIT $3",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Integer(i64::from(limit.min(1000)))]).await?;
    Ok((
        transaction,
        rows.into_iter()
            .map(decode_row)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}
