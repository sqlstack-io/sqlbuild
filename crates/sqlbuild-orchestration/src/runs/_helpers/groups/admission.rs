//! A group takes every child's pools when admitted, so no admitted run waits on another pool.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::runs::models::{LimitWaiter, QueueCandidate, RunKind, RunRequest};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::ControlTransaction;

/// A pool's slots in its run's location and target: pools belong to their project.
pub(crate) fn slot_key(request: &RunRequest, pool: &str) -> String {
    format!("{}/{}/{pool}", request.location_id, request.target)
}

/// The pool name of a slot key.
pub(crate) fn slot_pool(key: &str) -> &str {
    key.rsplit_once('/').map_or(key, |(_, pool)| pool)
}

/// The request a group is admitted with: its own and every child's pools, each at the largest amount.
pub(crate) fn admitted<'a>(
    request: &RunRequest,
    children: impl IntoIterator<Item = &'a RunRequest>,
) -> RunRequest {
    let mut admitted = request.clone();
    for child in children {
        for (pool, slots) in &child.pools {
            let held = admitted.pools.entry(pool.clone()).or_insert(*slots);
            held.amount = held.amount.max(slots.amount);
        }
    }
    admitted
}

/// Slots a run takes on admission; group children share their parent's, one child per limit at a time.
pub(crate) fn admission_limits<'a>(
    request: &RunRequest,
    children: impl IntoIterator<Item = &'a RunRequest>,
) -> BTreeSet<String> {
    if request.parent_run_id.is_some() {
        return BTreeSet::new();
    }
    admitted(request, children)
        .pools
        .keys()
        .map(|pool| slot_key(request, pool))
        .collect()
}

/// Queued top-level runs in dispatchable scopes, with the limits each will take.
pub(crate) async fn limit_waiters(
    transaction: ControlTransaction,
    eligible: Option<&BTreeSet<(Uuid, String)>>,
) -> Result<(ControlTransaction, Vec<LimitWaiter>), ControlError> {
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT r.request_json, r.queued_at FROM sqb_runs r WHERE r.status='queued' AND r.parent_run_id IS NULL \
         AND r.cancel_requested_at IS NULL AND r.worker_instance IS NULL",
        vec![]).await?;
    let waiting: Vec<QueueCandidate> = rows
        .into_iter()
        .map(decode_row)
        .collect::<Result<Vec<QueueCandidate>, _>>()?
        .into_iter()
        .filter(|run| run.request_json.due(now))
        .filter(|run| {
            eligible.is_none_or(|scopes| {
                scopes.contains(&(
                    run.request_json.location_id,
                    run.request_json.target.clone(),
                ))
            })
        })
        .collect();
    let (transaction, rows) = query(
        transaction,
        "SELECT c.request_json FROM sqb_runs c JOIN sqb_runs p ON p.run_id=c.parent_run_id \
         WHERE p.kind='group' AND p.status='queued' AND p.cancel_requested_at IS NULL",
        vec![],
    )
    .await?;
    let mut children: BTreeMap<Uuid, Vec<RunRequest>> = BTreeMap::new();
    for row in rows {
        let child: RunRequest = serde_json::from_value(row["request_json"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        if let Some(parent) = child.parent_run_id {
            children.entry(parent).or_default().push(child);
        }
    }
    let waiters: Vec<LimitWaiter> = waiting
        .into_iter()
        .map(|run| {
            let request = &run.request_json;
            let members = if request.kind == RunKind::Group {
                children
                    .get(&request.run_id)
                    .map(Vec::as_slice)
                    .unwrap_or_default()
            } else {
                &[]
            };
            LimitWaiter {
                rank: rank(&run),
                run_id: request.run_id,
                limits: admission_limits(request, members),
            }
        })
        .collect();
    Ok((transaction, waiters))
}

/// Strict per-limit queue order: an earlier waiter for any of the same limits goes first.
pub(crate) fn earlier_waiter_blocks(
    waiters: &[LimitWaiter],
    candidate: &QueueCandidate,
    limits: &BTreeSet<String>,
) -> bool {
    let position = rank(candidate);
    waiters
        .iter()
        .any(|waiter| ahead_on(waiter, (candidate, position), limits).is_some())
}

/// Waiters ranked ahead of the candidate on a limit it takes, each with the first limit they share.
pub(crate) fn earlier_waiters<'a>(
    waiters: &'a [LimitWaiter],
    candidate: &QueueCandidate,
    limits: &'a BTreeSet<String>,
) -> Vec<(&'a LimitWaiter, &'a String)> {
    let position = rank(candidate);
    let mut earlier: Vec<(&LimitWaiter, &String)> = Vec::new();
    for waiter in waiters {
        if let Some(limit) = ahead_on(waiter, (candidate, position), limits) {
            earlier.push((waiter, limit));
        }
    }
    earlier
}

/// The first of `limits` the waiter shares when it ranks ahead of the candidate.
fn ahead_on<'a>(
    waiter: &LimitWaiter,
    (candidate, position): (
        &QueueCandidate,
        (Reverse<i64>, chrono::DateTime<chrono::Utc>, Uuid),
    ),
    limits: &'a BTreeSet<String>,
) -> Option<&'a String> {
    if waiter.run_id == candidate.request_json.run_id || waiter.rank >= position {
        return None;
    }
    limits.iter().find(|limit| waiter.limits.contains(*limit))
}

fn rank(run: &QueueCandidate) -> (Reverse<i64>, chrono::DateTime<chrono::Utc>, Uuid) {
    (
        Reverse(run.request_json.priority),
        run.queued_at,
        run.request_json.run_id,
    )
}
