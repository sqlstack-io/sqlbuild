//! Bounded metadata projection; it never includes SQL, sampled rows or captured output.

use uuid::Uuid;

use std::collections::BTreeSet;

use crate::runs::_helpers::groups::gate::node_gates;
use crate::runs::_helpers::lifecycle::waits::queue_states;
use crate::runs::constants::NODE_PENDING;

use crate::runs::models::{NodeGate, RunNodeRecord, RunRecord, RunRequest, RunStatus};
use crate::runs::types::RunsListing;
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

pub(crate) async fn run_nodes_in(
    transaction: ControlTransaction,
    run_id: Uuid,
) -> Result<(ControlTransaction, Vec<RunNodeRecord>), ControlError> {
    let (transaction, nodes) = query_records(transaction,
        "SELECT node_key, status, started_at, ended_at, rows_affected, error_code, skipped_because_of \
         FROM sqb_run_nodes WHERE run_id=$1 AND kind='node' ORDER BY node_key",
        vec![Parameter::Id(run_id)]).await?;
    gate_waits(transaction, run_id, nodes).await
}

/// The run's request and status, for naming what its pending nodes wait for.
#[derive(serde::Deserialize)]
struct GatedRun {
    request_json: RunRequest,
    status: RunStatus,
}

/// Each pending node of a running group member that waits per node, with the inputs it still waits for.
async fn gate_waits(
    transaction: ControlTransaction,
    run_id: Uuid,
    mut nodes: Vec<RunNodeRecord>,
) -> Result<(ControlTransaction, Vec<RunNodeRecord>), ControlError> {
    let pending: BTreeSet<String> = nodes
        .iter()
        .filter(|node| node.status == NODE_PENDING)
        .map(|node| node.node_key.clone())
        .collect();
    if pending.is_empty() {
        return Ok((transaction, nodes));
    }
    let (transaction, runs): (ControlTransaction, Vec<GatedRun>) = query_records(
        transaction,
        "SELECT request_json, status FROM sqb_runs WHERE run_id=$1 AND parent_run_id IS NOT NULL",
        vec![Parameter::Id(run_id)],
    )
    .await?;
    let Some(run) = runs
        .into_iter()
        .next()
        .filter(|run| !run.status.is_terminal())
    else {
        return Ok((transaction, nodes));
    };
    let (transaction, mut gates) = node_gates(transaction, &run.request_json, &pending).await?;
    for node in &mut nodes {
        if let Some(NodeGate::Waiting { waiting_for }) = gates.remove(&node.node_key) {
            node.waiting_for = waiting_for;
        }
    }
    Ok((transaction, nodes))
}
