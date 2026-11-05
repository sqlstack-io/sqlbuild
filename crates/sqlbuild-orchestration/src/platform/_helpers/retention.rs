//! Every retention policy runs in one bounded pass; each part is safe to repeat.

use chrono::TimeDelta;

use crate::credentials::main::audit::prune_audit::prune_audit;
use crate::credentials::models::AuditRetention;
use crate::platform::constants::{
    HEARTBEAT_PAGE, HEARTBEAT_RETENTION_DAYS, SENSOR_EVALUATION_RETENTION_DAYS,
};
use crate::snapshots::main::prune_snapshots::prune_snapshots;
use crate::snapshots::models::SnapshotRetention;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::main::prune_statistics::prune_statistics;
use crate::warehouse_metadata::models::StatisticsRetention;

/// Returns how many audit rows outlive their horizon because a sink has not acknowledged them.
pub(crate) async fn retain(
    store: &ControlStore,
    (audit, statistics, snapshots): (AuditRetention, StatisticsRetention, SnapshotRetention),
    sinks: &[String],
) -> Result<i64, ControlError> {
    prune_snapshots(store, snapshots).await?;
    while prune_statistics(store, statistics).await? == MAX_HISTORY_RECORDS {
        tokio::task::yield_now().await;
    }
    while prune_heartbeats(store).await? == HEARTBEAT_PAGE {
        tokio::task::yield_now().await;
    }
    while prune_sensor_evaluations(store).await? == HEARTBEAT_PAGE {
        tokio::task::yield_now().await;
    }
    Ok(prune_audit(store, audit, sinks).await?.1)
}

/// Old evaluation phases go once a later attempt of the same sensor exists.
async fn prune_sensor_evaluations(store: &ControlStore) -> Result<usize, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let cutoff = transaction.now - TimeDelta::days(SENSOR_EVALUATION_RETENTION_DAYS);
    let (transaction, rows) = query(
        transaction,
        "DELETE FROM sqb_sensor_evaluation_events WHERE event_id IN (SELECT e.event_id \
         FROM sqb_sensor_evaluation_events e WHERE e.occurred_at<$1 AND EXISTS (SELECT 1 \
         FROM sqb_sensor_evaluation_events later WHERE later.trigger_id=e.trigger_id \
         AND later.phase='requested' AND later.occurred_at>e.occurred_at) \
         ORDER BY e.occurred_at LIMIT $2) RETURNING event_id",
        vec![
            Parameter::Time(cutoff),
            Parameter::Integer(HEARTBEAT_PAGE as i64),
        ],
    )
    .await?;
    commit(transaction).await?;
    Ok(rows.len())
}

/// Heartbeats only prove a replica was alive; alert facts stay for webhooks and sensors.
async fn prune_heartbeats(store: &ControlStore) -> Result<usize, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let cutoff = transaction.now - TimeDelta::days(HEARTBEAT_RETENTION_DAYS);
    let (transaction, rows) = query(
        transaction,
        "DELETE FROM recall_events WHERE storage_id IN (SELECT storage_id FROM recall_events \
         WHERE producer='platform' AND event_type='platform_heartbeat' AND occurred_at<$1 \
         ORDER BY storage_id LIMIT $2) RETURNING storage_id",
        vec![
            Parameter::Time(cutoff),
            Parameter::Integer(HEARTBEAT_PAGE as i64),
        ],
    )
    .await?;
    commit(transaction).await?;
    Ok(rows.len())
}
