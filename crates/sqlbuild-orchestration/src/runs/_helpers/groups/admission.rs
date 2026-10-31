//! Reconstruct global parent allocations and remaining child demands in the reservation transaction.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use uuid::Uuid;

use crate::runs::_helpers::groups::capacity::{claim_resources, parent_resources};
use crate::runs::_helpers::groups::selection::child_ready;
use crate::runs::models::{
    ConcurrencyResource, GroupCapacity, GroupRun, QueueCandidate, RunKind, RunStatus,
};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::ControlTransaction;

pub(crate) async fn group_capacity(
    transaction: ControlTransaction,
    limits: &BTreeMap<String, NonZeroU32>,
    eligible: Option<&BTreeSet<(Uuid, String)>>,
) -> Result<(ControlTransaction, GroupCapacity), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT r.request_json, r.queued_at, r.status, r.cancel_requested_at, r.node_counts_json, l.worker_instance FROM sqb_runs r \
         LEFT JOIN sqb_run_liveness l ON l.run_id=r.run_id \
         WHERE (r.kind='group' AND r.status IN ('queued','running')) \
         OR (r.status='queued' AND r.cancel_requested_at IS NULL AND l.run_id IS NULL) \
         ORDER BY r.priority DESC, r.queued_at, r.run_id", vec![]).await?;
    let runs: Vec<GroupRun> = rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    let parents: BTreeMap<_, _> = runs
        .iter()
        .filter(|run| run.request_json.kind == RunKind::Group)
        .map(|run| (run.request_json.run_id, run))
        .collect();
    let (transaction, slots) = query(transaction,
        "SELECT s.group_key, s.run_id FROM sqb_concurrency_slots s JOIN sqb_runs r ON r.run_id=s.run_id WHERE r.kind='group' AND r.status='running'", vec![]).await?;
    let mut capacity = GroupCapacity {
        available: limits
            .iter()
            .map(|(key, limit)| {
                (
                    ConcurrencyResource::Limit(key.clone()),
                    i64::from(limit.get()),
                )
            })
            .collect(),
        held: BTreeMap::new(),
        needed: parents.keys().map(|id| (*id, BTreeSet::new())).collect(),
        waiting: runs
            .iter()
            .filter(|run| {
                run.state.status == RunStatus::Queued
                    && run.state.cancel_requested_at.is_none()
                    && eligible.is_none_or(|scopes| {
                        scopes.contains(&(
                            run.request_json.location_id,
                            run.request_json.target.clone(),
                        ))
                    })
                    && run.request_json.parent_run_id.is_none_or(|id| {
                        parents.get(&id).is_some_and(|parent| {
                            parent.state.status == RunStatus::Running
                                && parent.state.cancel_requested_at.is_none()
                        })
                    })
            })
            .map(|run| QueueCandidate {
                request_json: run.request_json.clone(),
                queued_at: run.queued_at,
            })
            .collect(),
    };
    for (id, parent) in &parents {
        if parent.state.status == RunStatus::Running {
            let held = capacity.held.entry(*id).or_default();
            held.extend(
                parent_resources(&parent.request_json)
                    .into_iter()
                    .filter(|resource| matches!(resource, ConcurrencyResource::Overlap(..))),
            );
        }
    }
    for slot in slots {
        let id = serde_json::from_value(slot["run_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let key = slot["group_key"]
            .as_str()
            .ok_or_else(|| ControlError::Invalid("concurrency claim has no group".into()))?;
        capacity
            .held
            .entry(id)
            .or_default()
            .insert(ConcurrencyResource::Limit(key.into()));
    }
    for child in runs
        .iter()
        .filter(|run| run.request_json.kind != RunKind::Group)
    {
        let Some(parent) = child
            .request_json
            .parent_run_id
            .and_then(|id| parents.get(&id))
        else {
            continue;
        };
        if parent.state.cancel_requested_at.is_some()
            || (parent.state.status == RunStatus::Queued
                && !capacity
                    .waiting
                    .iter()
                    .any(|waiter| waiter.request_json.run_id == parent.request_json.run_id))
        {
            continue;
        }
        let inherited = if parent.state.status == RunStatus::Running {
            capacity.held[&parent.request_json.run_id].clone()
        } else {
            parent_resources(&parent.request_json)
        };
        capacity
            .needed
            .entry(parent.request_json.run_id)
            .or_default()
            .extend(claim_resources(&child.request_json, Some(&inherited)));
    }
    for resource in capacity
        .held
        .values()
        .flatten()
        .chain(capacity.needed.values().flatten())
    {
        if !capacity.available.contains_key(resource) {
            match resource {
                ConcurrencyResource::Overlap(..) => {
                    capacity.available.insert(resource.clone(), 1);
                }
                ConcurrencyResource::Limit(key) => {
                    return Err(ControlError::Invalid(format!(
                        "concurrency group {key} has no configured limit"
                    )));
                }
            }
        }
    }
    for resource in capacity.held.values().flatten() {
        *capacity.available.entry(resource.clone()).or_insert(0) -= 1;
    }
    Ok((transaction, capacity))
}

/// Give earlier executable contenders first claim, without blocking children that drain parents.
pub(crate) async fn earlier_waiter_blocks(
    mut transaction: ControlTransaction,
    capacity: &GroupCapacity,
    candidate: &QueueCandidate,
) -> Result<(ControlTransaction, bool), ControlError> {
    if candidate.request_json.parent_run_id.is_some() {
        return Ok((transaction, false));
    }
    let footprint = parent_resources(&candidate.request_json);
    for older in capacity.earlier_waiters(candidate) {
        let inherited = older
            .request_json
            .parent_run_id
            .and_then(|id| capacity.held.get(&id));
        let mut waiting = claim_resources(&older.request_json, inherited);
        if candidate.request_json.kind == RunKind::Group {
            waiting.extend(
                capacity
                    .needed
                    .get(&older.request_json.run_id)
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
        }
        if footprint.is_disjoint(&waiting) {
            continue;
        }
        let ready;
        (transaction, ready) = child_ready(transaction, &older.request_json).await?;
        if ready {
            return Ok((transaction, true));
        }
    }
    Ok((transaction, false))
}
