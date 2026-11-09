//! Why queued runs wait, read with the admission rules the scheduler claims by; it reserves nothing.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::runs::_helpers::groups::admission::{earlier_waiters, limit_waiters};
use crate::runs::_helpers::groups::selection::child_waits;
use crate::runs::models::{
    LimitWaiter, QueueCandidate, QueueHolder, QueueWait, RunQueueState, RunRecord, RunningJob,
};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

/// Set each run's position and waits; `runs` are this scope's queued runs in claim order.
pub(crate) async fn queue_states(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    runs: &mut [RunRecord],
) -> Result<ControlTransaction, ControlError> {
    let scope = vec![Parameter::Id(location_id), Parameter::Text(target.into())];
    let (transaction, waiters) = limit_waiters(transaction, None).await?;
    let (transaction, rows) = query(transaction,
        "SELECT r.request_json, r.queued_at FROM sqb_runs r WHERE r.status='queued' AND r.worker_instance IS NULL AND r.location_id=$1 AND r.target=$2",
        scope.clone()).await?;
    let candidates: BTreeMap<Uuid, QueueCandidate> = decoded::<QueueCandidate>(rows)?
        .into_iter()
        .map(|run| (run.request_json.run_id, run))
        .collect();
    let (transaction, rows) = query(transaction,
        "SELECT w.node_key AS held, w.run_id, r.location_id, r.target FROM sqb_write_claims w JOIN sqb_runs r ON r.run_id=w.run_id WHERE w.target=$1",
        vec![Parameter::Text(target.into())]).await?;
    let claims: Vec<QueueHolder> = decoded(rows)?;
    let (transaction, rows) = query(transaction,
        "SELECT s.group_key AS held, s.run_id, r.location_id, r.target FROM sqb_concurrency_slots s JOIN sqb_runs r ON r.run_id=s.run_id",
        vec![]).await?;
    let slots: Vec<QueueHolder> = decoded(rows)?;
    let (mut transaction, rows) = query(transaction,
        "SELECT r.run_id, r.job_key FROM sqb_runs r WHERE r.location_id=$1 AND r.target=$2 AND r.job_key IS NOT NULL \
         AND (r.worker_instance IS NOT NULL OR (r.kind='group' AND r.status='running'))",
        scope).await?;
    let running: Vec<RunningJob> = decoded(rows)?;
    let shown = |holder: &QueueHolder| {
        (holder.location_id == location_id && holder.target == target).then_some(holder.run_id)
    };
    for (position, run) in (1..).zip(runs.iter_mut()) {
        let Some(candidate) = candidates.get(&run.run_id) else {
            continue;
        };
        let request = &candidate.request_json;
        let mut waits;
        (transaction, waits) = child_waits(transaction, request).await?;
        waits.extend(
            running
                .iter()
                .filter(|job| {
                    job.run_id != request.run_id
                        && request.job_key.as_deref() == Some(job.job_key.as_str())
                })
                .map(|job| QueueWait::JobRunning { run_id: job.run_id }),
        );
        waits.extend(write_claims(
            &claims,
            &shown,
            (request.run_id, &request.write_set),
        ));
        let limits: BTreeSet<String> = waiters
            .iter()
            .find(|waiter| waiter.run_id == request.run_id)
            .map(|waiter| waiter.limits.clone())
            .unwrap_or_default();
        waits.extend(held_limits(&slots, &shown, (request.run_id, &limits)));
        waits.extend(earlier_runs(&waiters, (candidate, &limits), &candidates));
        run.queue = Some(RunQueueState { position, waits });
    }
    Ok(transaction)
}

fn decoded<T: serde::de::DeserializeOwned>(rows: Vec<Row>) -> Result<Vec<T>, ControlError> {
    rows.into_iter().map(decode_row).collect()
}

/// One wait per other run holding nodes this run writes, naming its first node.
fn write_claims(
    claims: &[QueueHolder],
    shown: &impl Fn(&QueueHolder) -> Option<Uuid>,
    (run_id, write_set): (Uuid, &BTreeSet<String>),
) -> Vec<QueueWait> {
    let mut held: BTreeMap<(Option<Uuid>, Uuid), Vec<&str>> = BTreeMap::new();
    for claim in claims {
        if claim.run_id != run_id && write_set.contains(&claim.held) {
            held.entry((shown(claim), claim.run_id))
                .or_default()
                .push(&claim.held);
        }
    }
    held.into_iter()
        .map(|((shown, _), nodes)| QueueWait::WriteClaim {
            run_id: shown,
            node_key: nodes[0].to_owned(),
            other_nodes: count(nodes.len() - 1),
        })
        .collect()
}

/// Each limit this run takes that running runs hold, naming holders in this scope and counting the rest.
fn held_limits(
    slots: &[QueueHolder],
    shown: &impl Fn(&QueueHolder) -> Option<Uuid>,
    (run_id, limits): (Uuid, &BTreeSet<String>),
) -> Vec<QueueWait> {
    let mut waits: Vec<QueueWait> = Vec::new();
    for limit in limits {
        let held: Vec<&QueueHolder> = slots
            .iter()
            .filter(|slot| &slot.held == limit && slot.run_id != run_id)
            .collect();
        if held.is_empty() {
            continue;
        }
        let holders: Vec<Uuid> = held.iter().filter_map(|slot| shown(slot)).collect();
        waits.push(QueueWait::ConcurrencyLimit {
            limit: limit.clone(),
            elsewhere: count(held.len() - holders.len()),
            holders,
        });
    }
    waits
}

/// The first earlier waiter on each shared limit; one queued outside this scope is not named.
fn earlier_runs(
    waiters: &[LimitWaiter],
    (candidate, limits): (&QueueCandidate, &BTreeSet<String>),
    here: &BTreeMap<Uuid, QueueCandidate>,
) -> Vec<QueueWait> {
    let mut ahead: BTreeSet<&String> = BTreeSet::new();
    let mut waits: Vec<QueueWait> = Vec::new();
    for (waiter, limit) in earlier_waiters(waiters, candidate, limits) {
        if ahead.insert(limit) {
            waits.push(QueueWait::EarlierRun {
                run_id: here.contains_key(&waiter.run_id).then_some(waiter.run_id),
                limit: limit.clone(),
            });
        }
    }
    waits
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
