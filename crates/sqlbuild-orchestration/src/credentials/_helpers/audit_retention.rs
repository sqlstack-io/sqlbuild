//! Bounded audit expiry batches that never outrun a sink or the storage order.

use chrono::{DateTime, Utc};

use crate::credentials::constants::AUDIT_RETENTION_BATCH;
use crate::credentials::models::AuditRetention;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

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
            "SELECT a.storage_id FROM sqb_security_audit a \
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
    let expired: Vec<Parameter> = rows
        .iter()
        .filter_map(|row| row["storage_id"].as_i64())
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
