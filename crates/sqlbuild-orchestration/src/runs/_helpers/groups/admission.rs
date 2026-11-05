//! A group takes every child's limits when admitted, so no admitted run waits on another limit.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::runs::models::{LimitWaiter, QueueCandidate, RunKind, RunRequest};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::ControlTransaction;

/// Limits a run takes on admission; group children inherit their parent's and take none.
pub(crate) fn admission_limits<'a>(
    request: &RunRequest,
    children: impl IntoIterator<Item = &'a RunRequest>,
) -> BTreeSet<String> {
    if request.parent_run_id.is_some() {
        return BTreeSet::new();
    }
    let mut limits = request.concurrency_groups.clone();
    for child in children {
        limits.extend(child.concurrency_groups.iter().cloned());
    }
    limits
}

/// Queued top-level runs in dispatchable scopes, with the limits each will take.
pub(crate) async fn limit_waiters(
    transaction: ControlTransaction,
    eligible: Option<&BTreeSet<(Uuid, String)>>,
) -> Result<(ControlTransaction, Vec<LimitWaiter>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT r.request_json, r.queued_at FROM sqb_runs r WHERE r.status='queued' AND r.parent_run_id IS NULL \
         AND r.cancel_requested_at IS NULL AND NOT EXISTS (SELECT 1 FROM sqb_run_liveness l WHERE l.run_id=r.run_id)",
        vec![]).await?;
    let waiting: Vec<QueueCandidate> = rows
        .into_iter()
        .map(decode_row)
        .collect::<Result<Vec<QueueCandidate>, _>>()?
        .into_iter()
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
    waiters.iter().any(|waiter| {
        waiter.run_id != candidate.request_json.run_id
            && waiter.rank < position
            && !waiter.limits.is_disjoint(limits)
    })
}

fn rank(run: &QueueCandidate) -> (Reverse<i64>, chrono::DateTime<chrono::Utc>, Uuid) {
    (
        Reverse(run.request_json.priority),
        run.queued_at,
        run.request_json.run_id,
    )
}
