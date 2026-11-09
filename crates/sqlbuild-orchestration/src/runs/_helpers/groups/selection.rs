//! Failed nodes block only their lineage dependents inside a group.

use std::collections::{BTreeMap, BTreeSet};

use crate::runs::_helpers::groups::context::load_group;
use crate::runs::models::{QueueWait, RunRequest, RunStatus};
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn child_ready(
    transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, waits) = child_waits(transaction, request).await?;
    Ok((transaction, waits.is_empty()))
}

/// What holds a group child back: its group not running, or upstream jobs of the group not finished.
pub(crate) async fn child_waits(
    transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<(ControlTransaction, Vec<QueueWait>), ControlError> {
    let Some(parent_id) = request.parent_run_id else {
        return Ok((transaction, Vec::new()));
    };
    let (transaction, group) = load_group(transaction, parent_id).await?;
    let name = request
        .job_key
        .as_ref()
        .ok_or_else(|| ControlError::Invalid("group child has no job".into()))?;
    let parent_name = group
        .parent
        .request_json
        .job_key
        .as_ref()
        .ok_or_else(|| ControlError::Invalid("group has no declaration".into()))?;
    let dependencies = upstream_jobs(&group.definitions.groups[parent_name].jobs, name);
    if group.parent.state.status != RunStatus::Running
        || group.parent.state.cancel_requested_at.is_some()
        || group.stopped()
    {
        return Ok((
            transaction,
            vec![QueueWait::GroupParent { run_id: parent_id }],
        ));
    }
    let waits: Vec<QueueWait> = group
        .children
        .iter()
        .filter_map(|child| {
            let job_key = child.request_json.job_key.as_ref()?;
            (dependencies.contains(job_key.as_str()) && !child.state.status.is_terminal()).then(
                || QueueWait::UpstreamJob {
                    run_id: child.request_json.run_id,
                    job_key: job_key.clone(),
                },
            )
        })
        .collect();
    Ok((transaction, waits))
}

pub(crate) async fn skipped_nodes(
    transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<(ControlTransaction, BTreeMap<String, String>), ControlError> {
    let Some(parent_id) = request.parent_run_id else {
        return Ok((transaction, BTreeMap::new()));
    };
    let (transaction, rows) = query(transaction,
        "SELECT n.node_key, n.status, n.skipped_because_of FROM sqb_run_nodes n JOIN sqb_runs r ON r.run_id=n.run_id \
         WHERE r.parent_run_id=$1 AND r.run_id<>$2 AND n.kind='node' AND n.status IN ('failed','skipped','cancelled')",
        vec![Parameter::Id(parent_id), Parameter::Id(request.run_id)]).await?;
    let failures: BTreeMap<String, String> = rows
        .into_iter()
        .filter_map(|row| {
            let key = row.get("node_key")?.as_str()?.to_owned();
            let cause = row
                .get("skipped_because_of")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&key)
                .to_owned();
            Some((key, cause))
        })
        .collect();
    if failures.is_empty() {
        return Ok((transaction, BTreeMap::new()));
    }
    let (transaction, nodes) = read_snapshot_nodes(transaction, request.snapshot_id, None).await?;
    let mut skipped: BTreeMap<String, String> = BTreeMap::new();
    for selected in &request.selection {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut pending = vec![selected.as_str()];
        while let Some(key) = pending.pop() {
            if !seen.insert(key) {
                continue;
            }
            if let Some(cause) = failures.get(key) {
                skipped.insert(selected.clone(), cause.clone());
                break;
            }
            if let Some(node) = nodes.get(key)
                && let Some(deps) = node
                    .definition
                    .get("deps")
                    .and_then(serde_json::Value::as_array)
            {
                pending.extend(deps.iter().filter_map(serde_json::Value::as_str));
            }
        }
    }
    Ok((transaction, skipped))
}

/// Completed jobs omitted from a retry do not sever explicit ordering between retained jobs.
fn upstream_jobs<'a>(
    edges: &'a BTreeMap<String, BTreeSet<String>>,
    name: &str,
) -> BTreeSet<&'a str> {
    let mut ancestors: BTreeSet<&str> = BTreeSet::new();
    let mut pending: Vec<&str> = edges[name].iter().map(String::as_str).collect();
    while let Some(name) = pending.pop() {
        if ancestors.insert(name) {
            pending.extend(edges[name].iter().map(String::as_str));
        }
    }
    ancestors
}
