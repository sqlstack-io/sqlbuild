//! Installation API reads never mix locations or targets.

use uuid::Uuid;

use crate::runs::models::RunRecord;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Return recent projected runs; per-run events remain in Recall.
pub async fn list_runs(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    limit: u32,
) -> Result<Vec<RunRecord>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(transaction,
        "SELECT run_id, location_id, target, snapshot_id, job_key, status, queued_at, launched_at, started_at, ended_at, selection_json, node_counts_json, last_event_storage_id \
         FROM sqb_runs WHERE location_id=$1 AND target=$2 ORDER BY queued_at DESC, run_id DESC LIMIT $3",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Integer(i64::from(limit.min(1000)))]).await?;
    let runs = rows
        .into_iter()
        .map(decode_row)
        .collect::<Result<Vec<_>, _>>()?;
    commit(transaction).await?;
    Ok(runs)
}
