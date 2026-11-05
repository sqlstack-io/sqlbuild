//! Every retention policy runs in one bounded pass; each part is safe to repeat.

use chrono::TimeDelta;

use crate::credentials::main::audit::prune_audit::prune_audit;
use crate::credentials::models::AuditRetention;
use crate::platform::constants::{
    HEARTBEAT_PAGE, HEARTBEAT_RETENTION_DAYS, LOCATION_RETENTION_DAYS,
    PLATFORM_ALERT_RETENTION_DAYS, SENSOR_EVALUATION_RETENTION_DAYS,
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
    while prune_platform_events(store).await? == HEARTBEAT_PAGE {
        tokio::task::yield_now().await;
    }
    while prune_locations(store).await? == HEARTBEAT_PAGE {
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

/// Heartbeats only prove a replica was alive, so they go sooner than alerts.
async fn prune_platform_events(store: &ControlStore) -> Result<usize, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let heartbeats = transaction.now - TimeDelta::days(HEARTBEAT_RETENTION_DAYS);
    let alerts = transaction.now - TimeDelta::days(PLATFORM_ALERT_RETENTION_DAYS);
    let (transaction, rows) = query(
        transaction,
        "DELETE FROM recall_events WHERE storage_id IN (SELECT storage_id FROM recall_events \
         WHERE producer='platform' AND occurred_at<CASE WHEN event_type='platform_heartbeat' \
         THEN $1 ELSE $2 END ORDER BY storage_id LIMIT $3) RETURNING storage_id",
        vec![
            Parameter::Time(heartbeats),
            Parameter::Time(alerts),
            Parameter::Integer(HEARTBEAT_PAGE as i64),
        ],
    )
    .await?;
    commit(transaction).await?;
    Ok(rows.len())
}

/// Expired location work goes first, then instances that no remaining work names.
async fn prune_locations(store: &ControlStore) -> Result<usize, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let cutoff = transaction.now - TimeDelta::days(LOCATION_RETENTION_DAYS);
    let (transaction, work) = query(
        transaction,
        "DELETE FROM sqb_location_work WHERE request_id IN (SELECT request_id FROM sqb_location_work \
         WHERE expires_at<$1 ORDER BY expires_at LIMIT $2) RETURNING request_id",
        vec![
            Parameter::Time(cutoff),
            Parameter::Integer(HEARTBEAT_PAGE as i64),
        ],
    )
    .await?;
    let (transaction, instances) = query(
        transaction,
        "DELETE FROM sqb_location_instances WHERE instance_id IN (SELECT i.instance_id \
         FROM sqb_location_instances i WHERE i.last_seen_at<$1 AND NOT EXISTS (SELECT 1 \
         FROM sqb_location_work w WHERE w.session_id=i.instance_id) \
         ORDER BY i.last_seen_at LIMIT $2) RETURNING instance_id",
        vec![
            Parameter::Time(cutoff),
            Parameter::Integer(HEARTBEAT_PAGE as i64),
        ],
    )
    .await?;
    commit(transaction).await?;
    Ok(work.len().max(instances.len()))
}
