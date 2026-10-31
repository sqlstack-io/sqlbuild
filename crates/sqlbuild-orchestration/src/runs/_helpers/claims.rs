//! A rejected candidate rolls back its entire savepoint, including any partial write set.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use crate::runs::models::RunRequest;
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
            "SELECT r.run_id FROM sqb_runs r JOIN sqb_run_liveness l ON l.run_id=r.run_id \
             WHERE r.location_id=$1 AND r.target=$2 AND r.job_key=$3 LIMIT 1",
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
