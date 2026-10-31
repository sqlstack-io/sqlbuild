//! Reserve write claims and concurrency slots inside the candidate's rollback savepoint.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use serde_json::json;
use uuid::Uuid;

use crate::runs::_helpers::lifecycle::events::append_run_event;
use crate::runs::models::{RunClaim, RunRequest};
use crate::scheduler::models::Overlap;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Occupied slots above a lowered limit still count against the run's concurrency group.
pub(crate) async fn reserve_claims(
    mut transaction: ControlTransaction,
    request: &RunRequest,
    limits: &BTreeMap<String, NonZeroU32>,
) -> Result<(ControlTransaction, bool), ControlError> {
    if request.overlap != Overlap::Allow
        && let Some(job_key) = &request.job_key
    {
        let rows;
        (transaction, rows) = query(
            transaction,
            "SELECT r.run_id FROM sqb_runs r LEFT JOIN sqb_run_liveness l ON l.run_id=r.run_id \
             WHERE r.location_id=$1 AND r.target=$2 AND r.job_key=$3 \
             AND (l.run_id IS NOT NULL OR (r.kind='group' AND r.status='running')) LIMIT 1",
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
    let now = transaction.now;
    for node in &request.write_set {
        let inserted;
        (transaction, inserted) = query(transaction,
            "INSERT INTO sqb_write_claims (target, node_key, run_id, claimed_at) VALUES ($1,$2,$3,$4) \
             ON CONFLICT (target, node_key) DO NOTHING RETURNING node_key",
            vec![Parameter::Text(request.target.clone()), Parameter::Text(node.clone()), Parameter::Id(request.run_id), Parameter::Time(now)]).await?;
        if inserted.is_empty() {
            return Ok((transaction, false));
        }
    }
    for group in &request.concurrency_groups {
        if let Some(parent_id) = request.parent_run_id {
            let inherited;
            (transaction, inherited) = query(
                transaction,
                "SELECT slot FROM sqb_concurrency_slots WHERE group_key=$1 AND run_id=$2",
                vec![Parameter::Text(group.clone()), Parameter::Id(parent_id)],
            )
            .await?;
            if !inherited.is_empty() {
                continue;
            }
        }
        let limit = limits
            .get(group)
            .ok_or_else(|| {
                ControlError::Invalid(format!("concurrency group {group} has no configured limit"))
            })?
            .get();
        let slots;
        (transaction, slots) = query(
            transaction,
            "SELECT slot FROM sqb_concurrency_slots WHERE group_key=$1 ORDER BY slot",
            vec![Parameter::Text(group.clone())],
        )
        .await?;
        let occupied: BTreeSet<i64> = slots
            .iter()
            .filter_map(|row| row.get("slot").and_then(serde_json::Value::as_i64))
            .collect();
        if occupied.len() >= limit as usize {
            return Ok((transaction, false));
        }
        let slot = (1..=i64::from(limit))
            .find(|slot| !occupied.contains(slot))
            .ok_or_else(|| {
                ControlError::Invalid("concurrency slot accounting is inconsistent".into())
            })?;
        let inserted;
        (transaction, inserted) = query(transaction,
            "INSERT INTO sqb_concurrency_slots (group_key, slot, run_id, claimed_at) VALUES ($1,$2,$3,$4) \
             ON CONFLICT (group_key, slot) DO NOTHING RETURNING slot",
            vec![Parameter::Text(group.clone()), Parameter::Integer(slot), Parameter::Id(request.run_id), Parameter::Time(now)]).await?;
        if inserted.is_empty() {
            return Ok((transaction, false));
        }
    }
    Ok((transaction, true))
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
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_run_liveness (run_id, worker_instance, claimed_at, last_heartbeat_at) VALUES ($1,$2,$3,$3)",
        vec![Parameter::Id(claim.request.run_id), Parameter::Id(claim.claim_id), Parameter::Time(now)]).await?;
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
