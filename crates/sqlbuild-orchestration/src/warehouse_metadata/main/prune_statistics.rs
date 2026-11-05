//! Expire statistics samples past their retention horizon in bounded pages.

use crate::store::errors::ControlError;
use crate::store::main::{query::query, write::write};
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::StatisticsRetention;

/// Expiration is bounded global cleanup, never a lifecycle status or warehouse write.
pub(crate) async fn prune_statistics(
    store: &ControlStore,
    retention: StatisticsRetention,
) -> Result<usize, ControlError> {
    write(store, |transaction| async move {
        let cutoff = retention.cutoff(transaction.now);
        let (transaction, rows) = query(transaction,
            "DELETE FROM sqb_warehouse_statistics WHERE event_id IN (SELECT event_id FROM sqb_warehouse_statistics \
             WHERE observed_at<$1 ORDER BY observed_at,event_id LIMIT $2) RETURNING event_id",
            vec![Parameter::Time(cutoff), Parameter::Integer(MAX_HISTORY_RECORDS as i64)]).await?;
        Ok((transaction, rows.len()))
    })
    .await
}
