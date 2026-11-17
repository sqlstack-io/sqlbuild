//! Reserve write claims and pool slots inside the candidate's rollback savepoint, all or none.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use serde_json::json;
use uuid::Uuid;

use crate::declarations::models::PoolSlots;
use crate::runs::_helpers::groups::admission::slot_key;
use crate::runs::_helpers::lifecycle::events::append_run_event;
use crate::runs::models::{RunClaim, RunRequest};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

/// Occupied slots above a lowered limit still count; group children use a parent's slot one at a time.
pub(crate) async fn reserve_claims(
    mut transaction: ControlTransaction,
    request: &RunRequest,
    caps: &BTreeMap<String, NonZeroU32>,
) -> Result<(ControlTransaction, bool), ControlError> {
    if let Some(job_key) = &request.job_key {
        let rows;
        (transaction, rows) = query(
            transaction,
            "SELECT r.run_id FROM sqb_runs r \
             WHERE r.location_id=$1 AND r.target=$2 AND r.job_key=$3 \
             AND (r.worker_instance IS NOT NULL OR (r.kind='group' AND r.status='running')) LIMIT 1",
            vec![
                Parameter::Id(request.location_id),
                Parameter::Text(request.target.clone()),
                Parameter::Text(job_key.clone()),
            ],
        )
        .await?;
        if !rows.is_empty() {
            return Ok((transaction, false));
        }
    }
    let mut siblings_hold: BTreeSet<String> = BTreeSet::new();
    if let Some(parent_id) = request.parent_run_id {
        let rows;
        (transaction, rows) = query(
            transaction,
            "SELECT request_json FROM sqb_runs WHERE parent_run_id=$1 AND run_id<>$2 AND worker_instance IS NOT NULL",
            vec![Parameter::Id(parent_id), Parameter::Id(request.run_id)],
        )
        .await?;
        for row in rows {
            let sibling: RunRequest = serde_json::from_value(row["request_json"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            siblings_hold.extend(sibling.pools.into_keys());
        }
    }
    for node in &request.write_set {
        let inserted;
        (transaction, inserted) = query(
            transaction,
            "INSERT INTO sqb_write_claims (target, node_key, run_id) VALUES ($1,$2,$3) \
             ON CONFLICT (target, node_key) DO NOTHING RETURNING node_key",
            vec![
                Parameter::Text(request.target.clone()),
                Parameter::Text(node.clone()),
                Parameter::Id(request.run_id),
            ],
        )
        .await?;
        if inserted.is_empty() {
            return Ok((transaction, false));
        }
    }
    for (pool, slots) in &request.pools {
        let key = slot_key(request, pool);
        if let Some(parent_id) = request.parent_run_id {
            let inherited;
            (transaction, inherited) = query(
                transaction,
                "SELECT slot FROM sqb_concurrency_slots WHERE group_key=$1 AND run_id=$2",
                vec![Parameter::Text(key.clone()), Parameter::Id(parent_id)],
            )
            .await?;
            if !inherited.is_empty() {
                if siblings_hold.contains(pool) {
                    return Ok((transaction, false));
                }
                continue;
            }
        }
        let occupied;
        (transaction, occupied) = query(
            transaction,
            "SELECT slot FROM sqb_concurrency_slots WHERE group_key=$1 ORDER BY slot",
            vec![Parameter::Text(key.clone())],
        )
        .await?;
        let Some(free) = free_slots(&occupied, effective_slots(*slots, caps.get(pool))) else {
            return Ok((transaction, false));
        };
        for slot in free {
            let inserted;
            (transaction, inserted) = query(
                transaction,
                "INSERT INTO sqb_concurrency_slots (group_key, slot, run_id) VALUES ($1,$2,$3) \
                 ON CONFLICT (group_key, slot) DO NOTHING RETURNING slot",
                vec![
                    Parameter::Text(key.clone()),
                    Parameter::Integer(slot),
                    Parameter::Id(request.run_id),
                ],
            )
            .await?;
            if inserted.is_empty() {
                return Ok((transaction, false));
            }
        }
    }
    Ok((transaction, true))
}

/// The run's amount and the pool's limit, lowered to the server's cap; above the cap a run takes the whole pool.
fn effective_slots(slots: PoolSlots, cap: Option<&NonZeroU32>) -> (u32, u32) {
    let limit = cap.map_or(slots.limit, |cap| slots.limit.min(*cap)).get();
    (slots.amount.get().min(limit), limit)
}

/// `amount` slot numbers free below `limit`, or `None` when the pool lacks room; slots above a lowered limit still count.
fn free_slots(occupied: &[Row], (amount, limit): (u32, u32)) -> Option<Vec<i64>> {
    let taken: BTreeSet<i64> = occupied
        .iter()
        .filter_map(|row| row.get("slot").and_then(serde_json::Value::as_i64))
        .collect();
    if taken.len() + amount as usize > limit as usize {
        return None;
    }
    let free: Vec<i64> = (1..=i64::from(limit))
        .filter(|slot| !taken.contains(slot))
        .take(amount as usize)
        .collect();
    (free.len() == amount as usize).then_some(free)
}

/// Publish ownership only after every physical claim has been reserved in this transaction.
pub(crate) async fn publish_claim(
    transaction: ControlTransaction,
    request: RunRequest,
) -> Result<(ControlTransaction, RunClaim), ControlError> {
    let claim = RunClaim {
        claim_id: Uuid::new_v5(&request.run_id, b"run_claimed"),
        request,
    };
    let now = transaction.now;
    let (transaction, claimed) = query(
        transaction,
        "UPDATE sqb_runs SET worker_instance=$2, claimed_at=$3, last_heartbeat_at=$3 \
         WHERE run_id=$1 AND worker_instance IS NULL RETURNING run_id",
        vec![
            Parameter::Id(claim.request.run_id),
            Parameter::Id(claim.claim_id),
            Parameter::Time(now),
        ],
    )
    .await?;
    if claimed.is_empty() {
        return Err(ControlError::Invalid("run is already claimed".into()));
    }
    let (transaction, cursor) = append_run_event(
        transaction,
        (claim.request.run_id, claim.claim_id),
        "run_claimed",
        json!({"claim": claim}),
    )
    .await?;
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_runs SET last_event_storage_id=$1 WHERE run_id=$2",
        vec![
            Parameter::Integer(cursor),
            Parameter::Id(claim.request.run_id),
        ],
    )
    .await?;
    Ok((transaction, claim))
}
