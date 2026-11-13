//! Every retention policy runs in one bounded pass; each part is safe to repeat.

use chrono::{DateTime, TimeDelta, Utc};

use crate::credentials::main::audit::prune_audit::prune_audit;
use crate::credentials::models::AuditRetention;
use crate::platform::constants::{
    HEARTBEAT_PAGE, HEARTBEAT_RETENTION_DAYS, LOCATION_RETENTION_DAYS,
    PLATFORM_ALERT_RETENTION_DAYS, SENSOR_EVALUATION_RETENTION_DAYS,
};
use crate::runs::main::prune_run_logs::prune_run_logs;
use crate::runs::models::RunLogRetention;
use crate::snapshots::main::prune_snapshots::prune_snapshots;
use crate::snapshots::models::SnapshotRetention;
use crate::store::errors::ControlError;
use crate::store::main::{query::query, write::write};
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::main::ingest::prune_statistics::prune_statistics;
use crate::warehouse_metadata::models::StatisticsRetention;

/// Old evaluation phases go once a later attempt of the same sensor exists.
const SENSOR_EVALUATIONS: &str = "DELETE FROM sqb_sensor_evaluation_events WHERE event_id IN (SELECT e.event_id \
    FROM sqb_sensor_evaluation_events e WHERE e.occurred_at<$1 AND EXISTS (SELECT 1 \
    FROM sqb_sensor_evaluation_events later WHERE later.trigger_id=e.trigger_id \
    AND later.phase='requested' AND later.occurred_at>e.occurred_at) \
    ORDER BY e.occurred_at LIMIT $2) RETURNING event_id";

/// Heartbeats only prove a replica was alive, so they go sooner than alerts.
const PLATFORM_EVENTS: &str = "DELETE FROM recall_events WHERE storage_id IN (SELECT storage_id FROM recall_events \
    WHERE producer='platform' AND occurred_at<CASE WHEN event_type='platform_heartbeat' \
    THEN $1 ELSE $2 END ORDER BY storage_id LIMIT $3) RETURNING storage_id";

/// Expired location work goes first, then instances that no remaining work names.
const LOCATION_WORK: &str = "DELETE FROM sqb_location_work WHERE request_id IN (SELECT request_id \
    FROM sqb_location_work WHERE expires_at<$1 ORDER BY expires_at LIMIT $2) RETURNING request_id";
const LOCATION_INSTANCES: &str = "DELETE FROM sqb_location_instances WHERE instance_id IN (SELECT i.instance_id \
    FROM sqb_location_instances i WHERE i.last_seen_at<$1 AND NOT EXISTS (SELECT 1 \
    FROM sqb_location_work w WHERE w.session_id=i.instance_id) \
    ORDER BY i.last_seen_at LIMIT $2) RETURNING instance_id";

/// Returns how many audit rows outlive their horizon because a sink has not acknowledged them.
pub(crate) async fn retain(
    store: &ControlStore,
    (audit, statistics, snapshots, run_logs): (
        AuditRetention,
        StatisticsRetention,
        SnapshotRetention,
        RunLogRetention,
    ),
    sinks: &[String],
) -> Result<i64, ControlError> {
    prune_run_logs(store, run_logs).await?;
    prune_snapshots(store, snapshots).await?;
    while prune_statistics(store, statistics).await? == MAX_HISTORY_RECORDS {
        tokio::task::yield_now().await;
    }
    drain(store, PLATFORM_EVENTS, |now| {
        vec![
            Parameter::Time(now - TimeDelta::days(HEARTBEAT_RETENTION_DAYS)),
            Parameter::Time(now - TimeDelta::days(PLATFORM_ALERT_RETENTION_DAYS)),
        ]
    })
    .await?;
    for sql in [LOCATION_WORK, LOCATION_INSTANCES] {
        drain(store, sql, |now| {
            vec![Parameter::Time(
                now - TimeDelta::days(LOCATION_RETENTION_DAYS),
            )]
        })
        .await?;
    }
    drain(store, SENSOR_EVALUATIONS, |now| {
        vec![Parameter::Time(
            now - TimeDelta::days(SENSOR_EVALUATION_RETENTION_DAYS),
        )]
    })
    .await?;
    Ok(prune_audit(store, audit, sinks).await?.1)
}

/// Deletes one page per writer transaction until a page comes back short, so the lock is never held long.
async fn drain(
    store: &ControlStore,
    sql: &str,
    cutoffs: impl Fn(DateTime<Utc>) -> Vec<Parameter>,
) -> Result<(), ControlError> {
    loop {
        let deleted = write(store, |transaction| async {
            let mut parameters = cutoffs(transaction.now);
            parameters.push(Parameter::Integer(HEARTBEAT_PAGE as i64));
            let (transaction, rows) = query(transaction, sql, parameters).await?;
            Ok((transaction, rows.len()))
        })
        .await?;
        if deleted < HEARTBEAT_PAGE {
            return Ok(());
        }
        tokio::task::yield_now().await;
    }
}
