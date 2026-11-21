//! Bounded metadata projection; it never includes SQL, sampled rows or captured output.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::runs::_helpers::lifecycle::waits::queue_states;

use crate::runs::models::{RunNodeRecord, RunRecord};
use crate::runs::types::RunsListing;
use crate::snapshots::main::reads::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

/// Newest first after an optional cursor, or the queue in claim order with each run's queue state.
pub(crate) async fn list_runs_in(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    listing: RunsListing,
) -> Result<(ControlTransaction, Vec<RunRecord>), ControlError> {
    let (limit, before) = match listing {
        RunsListing::Newest(limit, before) => (limit, before),
        RunsListing::Queue(limit) => (limit, None),
    };
    let mut parameters = vec![
        Parameter::Id(location_id),
        Parameter::Text(target.into()),
        Parameter::Integer(i64::from(limit.min(1000))),
    ];
    let filter = match (listing, before) {
        (RunsListing::Queue(_), _) => {
            "AND r.status='queued' AND r.worker_instance IS NULL ORDER BY r.priority DESC, r.queued_at, r.run_id"
        }
        (_, Some((queued_at, run_id))) => {
            parameters.extend([Parameter::Time(queued_at), Parameter::Id(run_id)]);
            "AND (r.queued_at<$4 OR (r.queued_at=$4 AND r.run_id<$5)) ORDER BY r.queued_at DESC, r.run_id DESC"
        }
        (_, None) => "ORDER BY r.queued_at DESC, r.run_id DESC",
    };
    let (transaction, mut runs) = query_records(transaction, &format!(
        "SELECT r.run_id, r.location_id, r.target, r.snapshot_id, s.generation AS snapshot_generation, s.git_revision, \
         s.git_dirty, s.sqb_version, r.image_id, r.job_key, r.kind, r.origin, r.parent_run_id, r.root_run_id, \
         r.trigger_id, t.name AS trigger_name, r.tick_id, r.rerun_of_run_id, r.requested_by, p.name AS requester_name, r.priority, \
         r.rewrite_history, r.status, r.status_reason, r.queued_at, r.launched_at, r.started_at, r.ended_at, r.cancel_requested_at, \
         r.selection_json, r.node_counts_json, r.ignored_audit_failures, r.last_event_storage_id, r.request_json AS narrowing, r.request_json AS backfill_window, r.request_json AS member_attempt \
         FROM sqb_runs r LEFT JOIN sqb_triggers t ON t.trigger_id=r.trigger_id \
         LEFT JOIN sqb_snapshots s ON s.snapshot_id=r.snapshot_id \
         LEFT JOIN sqb_principals p ON 'principal:' || CAST(p.principal_id AS TEXT)=r.requested_by \
         WHERE r.location_id=$1 AND r.target=$2 {filter} LIMIT $3"),
        parameters).await?;
    if !matches!(listing, RunsListing::Queue(_)) {
        return Ok((transaction, runs));
    }
    let transaction = queue_states(transaction, (location_id, target), &mut runs).await?;
    Ok((transaction, runs))
}

/// The run's nodes, each with its materialization in the snapshot the run was pinned to.
pub(crate) async fn run_nodes_in(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<(ControlTransaction, Vec<RunNodeRecord>), ControlError> {
    let (transaction, mut nodes): (_, Vec<RunNodeRecord>) = query_records(transaction,
        "SELECT node_key, status, started_at, ended_at, rows_affected, table_rows, rows_so_far, no_change_reason, \
         error_code, skipped_because_of FROM sqb_run_nodes WHERE run_id=$1 AND kind='node' ORDER BY node_key",
        vec![Parameter::Id(run_id)]).await?;
    let (transaction, runs): (_, Vec<BTreeMap<String, Uuid>>) = query_records(
        transaction,
        "SELECT snapshot_id FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(run_id)],
    )
    .await?;
    let Some(snapshot_id) = runs.first().and_then(|row| row.get("snapshot_id")).copied() else {
        return Ok((transaction, nodes));
    };
    let selection: BTreeSet<String> = nodes.iter().map(|node| node.node_key.clone()).collect();
    let (transaction, definitions) =
        read_snapshot_nodes(transaction, snapshot_id, Some(&selection)).await?;
    for node in &mut nodes {
        node.materialization = definitions
            .get(&node.node_key)
            .and_then(|definition| definition.definition.get("materialization"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
    }
    Ok((transaction, nodes))
}
