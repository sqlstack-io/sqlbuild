//! Failed nodes block only their lineage dependents unless an upstream job requests broader stops.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::models::OnFailure;
use crate::runs::_helpers::groups::context::load_group;
use crate::runs::models::{RunRequest, RunStatus};
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn child_ready(
    transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<(ControlTransaction, bool), ControlError> {
    let Some(parent_id) = request.parent_run_id else {
        return Ok((transaction, true));
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
    let dependencies = &group.definitions.groups[parent_name].jobs[name];
    let ready = group.parent.state.status == RunStatus::Running
        && group.parent.state.cancel_requested_at.is_none()
        && !group.stopped()
        && group.children.iter().all(|child| {
            child
                .request_json
                .job_key
                .as_ref()
                .is_none_or(|name| !dependencies.contains(name))
                || child.state.status.is_terminal()
        });
    Ok((transaction, ready))
}

pub(crate) async fn skipped_nodes(
    transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<(ControlTransaction, BTreeMap<String, String>), ControlError> {
    let Some(parent_id) = request.parent_run_id else {
        return Ok((transaction, BTreeMap::new()));
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
    let edges = &group.definitions.groups[parent_name].jobs;
    let mut ancestors: BTreeSet<&str> = BTreeSet::new();
    let mut pending: Vec<&str> = edges[name].iter().map(String::as_str).collect();
    while let Some(name) = pending.pop() {
        if ancestors.insert(name) {
            pending.extend(edges[name].iter().map(String::as_str));
        }
    }
    let (transaction, rows) = query(transaction,
        "SELECT n.node_key, n.status, n.skipped_because_of FROM sqb_run_nodes n JOIN sqb_runs r ON r.run_id=n.run_id \
         WHERE r.parent_run_id=$1 AND r.run_id<>$2 AND n.status IN ('failed','skipped','cancelled')",
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
    let whole_job = group.children.iter().find(|child| {
        child.request_json.job_key.as_ref().is_some_and(|name| {
            ancestors.contains(name.as_str())
                && group.definitions.jobs[name].on_failure == OnFailure::SkipDownstreamJobs
        }) && child.state.status.is_terminal()
            && !matches!(child.state.status, RunStatus::Success | RunStatus::Warn)
    });
    if let Some(child) = whole_job {
        let cause = child
            .request_json
            .selection
            .iter()
            .find_map(|key| failures.get(key))
            .cloned()
            .unwrap_or_else(|| format!("run:{}", child.request_json.run_id));
        return Ok((
            transaction,
            request
                .selection
                .iter()
                .map(|key| (key.clone(), cause.clone()))
                .collect(),
        ));
    }
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
