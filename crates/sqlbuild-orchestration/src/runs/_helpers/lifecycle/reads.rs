//! Bounded metadata projection; it never includes SQL, sampled rows or captured output.

use uuid::Uuid;

use crate::runs::models::{RunNodeRecord, RunRecord};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn list_runs_in(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    limit: u32,
) -> Result<(ControlTransaction, Vec<RunRecord>), ControlError> {
    query_records(transaction,
        "SELECT run_id, location_id, target, snapshot_id, job_key, status, queued_at, launched_at, started_at, ended_at, selection_json, node_counts_json, last_event_storage_id \
         FROM sqb_runs WHERE location_id=$1 AND target=$2 ORDER BY queued_at DESC, run_id DESC LIMIT $3",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Integer(i64::from(limit.min(1000)))]).await
}

pub(crate) async fn run_nodes_in(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<(ControlTransaction, Vec<RunNodeRecord>), ControlError> {
    query_records(transaction,
        "SELECT node_key, status, started_at, ended_at, rows_affected, error_code, skipped_because_of \
         FROM sqb_run_nodes WHERE run_id=$1 AND kind='node' ORDER BY node_key",
        vec![Parameter::Id(run_id)]).await
}

/// Whether `run_id` belongs to this location and target.
pub(crate) async fn run_in_scope(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    run_id: Uuid,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT run_id FROM sqb_runs WHERE run_id=$1 AND location_id=$2 AND target=$3",
        vec![
            Parameter::Id(run_id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    Ok((transaction, !rows.is_empty()))
}
