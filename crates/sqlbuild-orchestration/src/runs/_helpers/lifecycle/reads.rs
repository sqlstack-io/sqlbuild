//! Bounded metadata projection; it never includes SQL, sampled rows or captured output.

use uuid::Uuid;

use crate::runs::models::{RunNodeRecord, RunRecord};
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn list_runs_in(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    limit: u32,
) -> Result<(ControlTransaction, Vec<RunRecord>), ControlError> {
    query_records(transaction,
        "SELECT r.run_id, r.location_id, r.target, r.snapshot_id, r.job_key, r.kind, r.origin, r.parent_run_id, r.root_run_id, \
         r.trigger_id, t.name AS trigger_name, r.tick_id, r.rerun_of_run_id, r.requested_by, p.name AS requester_name, r.priority, \
         r.rewrite_history, r.status, r.status_reason, r.queued_at, r.launched_at, r.started_at, r.ended_at, r.cancel_requested_at, \
         r.selection_json, r.node_counts_json, r.last_event_storage_id \
         FROM sqb_runs r LEFT JOIN sqb_triggers t ON t.trigger_id=r.trigger_id \
         LEFT JOIN sqb_principals p ON 'principal:' || CAST(p.principal_id AS TEXT)=r.requested_by \
         WHERE r.location_id=$1 AND r.target=$2 ORDER BY r.queued_at DESC, r.run_id DESC LIMIT $3",
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
