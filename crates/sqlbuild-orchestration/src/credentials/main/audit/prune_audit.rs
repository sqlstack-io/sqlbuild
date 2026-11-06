//! Audit expiry is bounded cleanup that never outruns a sink, a retry lookup or the storage order.

use crate::credentials::_helpers::audit_retention::prune_audit_batch;
use crate::credentials::models::AuditRetention;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// One full pass in storage order; returns rows deleted and rows still kept past the horizon.
pub(crate) async fn prune_audit(
    store: &ControlStore,
    retention: AuditRetention,
    sinks: &[String],
) -> Result<(usize, i64), ControlError> {
    let mut deleted = 0;
    let mut after = 0;
    loop {
        let (count, next) = prune_audit_batch(store, (retention, sinks), after).await?;
        deleted += count;
        match next {
            Some(next) => after = next,
            None => break,
        }
    }
    let transaction = begin(store, TransactionMode::Read).await?;
    let cutoff = retention.cutoff(transaction.now);
    let (transaction, rows) = query(
        transaction,
        "SELECT COUNT(*) AS retained FROM sqb_security_audit WHERE occurred_at<$1",
        vec![Parameter::Time(cutoff)],
    )
    .await?;
    commit(transaction).await?;
    let retained = rows
        .first()
        .and_then(|row| row["retained"].as_i64())
        .unwrap_or_default();
    Ok((deleted, retained))
}
