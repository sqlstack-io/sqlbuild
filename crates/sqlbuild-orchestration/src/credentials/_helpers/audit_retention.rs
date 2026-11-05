//! Bounded audit expiry batches that never outrun a sink, a retry lookup or the storage order.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::credentials::constants::AUDIT_RETENTION_BATCH;
use crate::credentials::models::AuditRetention;
use crate::runs::constants::{MANUAL_JOB_AUDIT_TAG, RERUN_AUDIT_TAG};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::store::types::Row;

/// The newest row is never deleted, so the next MAX+1 storage ID is never reused.
pub(crate) async fn prune_audit_batch(
    store: &ControlStore,
    (retention, sinks): (AuditRetention, &[String]),
    after: i64,
) -> Result<(usize, Option<i64>), ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let cutoff: DateTime<Utc> = retention.cutoff(transaction.now);
    let acknowledged: String = (0..sinks.len())
        .map(|index| {
            format!(
                " AND EXISTS (SELECT 1 FROM sqb_security_audit_deliveries d \
                 WHERE d.sink_id=${} AND d.storage_id=a.storage_id)",
                index + 4
            )
        })
        .collect();
    let mut parameters = vec![
        Parameter::Time(cutoff),
        Parameter::Integer(after),
        Parameter::Integer(AUDIT_RETENTION_BATCH as i64),
    ];
    parameters.extend(sinks.iter().cloned().map(Parameter::Text));
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT a.storage_id, a.event_id, a.scope_json FROM sqb_security_audit a \
             WHERE a.occurred_at<$1 AND a.storage_id>$2 \
             AND a.storage_id<(SELECT MAX(storage_id) FROM sqb_security_audit){acknowledged} \
             ORDER BY a.storage_id LIMIT $3"
        ),
        parameters,
    )
    .await?;
    let next = (rows.len() == AUDIT_RETENTION_BATCH)
        .then(|| rows.last().and_then(|row| row["storage_id"].as_i64()))
        .flatten();
    let (transaction, referenced) = retry_referenced(transaction, &rows).await?;
    let expired: Vec<Parameter> = rows
        .iter()
        .filter_map(|row| row["storage_id"].as_i64())
        .filter(|storage_id| !referenced.contains(storage_id))
        .map(Parameter::Integer)
        .collect();
    let mut transaction = transaction;
    if !expired.is_empty() {
        let placeholders = (1..=expired.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(",");
        for table in ["sqb_security_audit_deliveries", "sqb_security_audit"] {
            (transaction, _) = query(
                transaction,
                &format!("DELETE FROM {table} WHERE storage_id IN ({placeholders})"),
                expired.clone(),
            )
            .await?;
        }
    }
    commit(transaction).await?;
    Ok((expired.len(), next))
}

/// Mirrors the retry join; a row whose identifiers cannot be read is kept, not guessed about.
async fn retry_referenced(
    transaction: ControlTransaction,
    rows: &[Row],
) -> Result<(ControlTransaction, BTreeSet<i64>), ControlError> {
    let mut kept: BTreeSet<i64> = BTreeSet::new();
    let mut candidates: Vec<(i64, Uuid)> = Vec::new();
    for row in rows {
        let Some(storage_id) = row["storage_id"].as_i64() else {
            continue;
        };
        let Some(run_id) = row["scope_json"].get("run_id") else {
            continue;
        };
        let (Ok(event_id), Ok(run_id)) = (
            serde_json::from_value::<Uuid>(row["event_id"].clone()),
            serde_json::from_value::<Uuid>(run_id.clone()),
        ) else {
            kept.insert(storage_id);
            continue;
        };
        if Uuid::new_v5(&run_id, MANUAL_JOB_AUDIT_TAG) == event_id
            || Uuid::new_v5(&run_id, RERUN_AUDIT_TAG) == event_id
        {
            candidates.push((storage_id, run_id));
        }
    }
    if candidates.is_empty() {
        return Ok((transaction, kept));
    }
    let placeholders = (1..=candidates.len())
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(",");
    let (transaction, runs) = query(
        transaction,
        &format!("SELECT run_id FROM sqb_runs WHERE run_id IN ({placeholders})"),
        candidates
            .iter()
            .map(|(_, run_id)| Parameter::Id(*run_id))
            .collect(),
    )
    .await?;
    let mut retained: BTreeSet<Uuid> = BTreeSet::new();
    for row in runs {
        retained.insert(
            serde_json::from_value(row["run_id"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?,
        );
    }
    kept.extend(
        candidates
            .into_iter()
            .filter(|(_, run_id)| retained.contains(run_id))
            .map(|(storage_id, _)| storage_id),
    );
    Ok((transaction, kept))
}
