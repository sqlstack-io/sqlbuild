//! Every retention policy runs in one bounded pass; each part is safe to repeat.

use std::collections::BTreeSet;
use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use rusqlite::config::DbConfig;
use sqlbuild_recall::schema::constants::INVOCATION_TABLES;
use sqlbuild_recall::schema::main::table_spec::table_spec;

use crate::credentials::main::audit::prune_audit::prune_audit;
use crate::platform::constants::{
    CHECKPOINT_READER_WAIT_MS, HEARTBEAT_PAGE, HEARTBEAT_RETENTION_DAYS, LOCATION_RETENTION_DAYS,
    PLATFORM_ALERT_RETENTION_DAYS, SENSOR_EVALUATION_RETENTION_DAYS,
};
use crate::platform::models::RetentionPolicies;
use crate::runs::constants::{RUN_LOG_DIGEST_CHUNK, RUN_LOG_RETENTION_BATCH};
use crate::runs::models::RunLogRetention;
use crate::snapshots::main::prune_snapshots::prune_snapshots;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query, write::write};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::main::ingest::prune_statistics::prune_statistics;

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

/// Claims of finished runs past either limit whose worker log is still stored, oldest first. Worker invocations are `worker/<claim>` and `worker/<claim>/<session>`; the stored logs, not every run ever finished, bound the join. A finished run no longer names its worker, so the scheduler's `run_claimed` event, whose ID is the claim, links the two.
const EXPIRED_CLAIMS: &str = "WITH logged AS (\
    SELECT DISTINCT substr(invocation_id, 8, 36) AS claim FROM recall_invocations \
    WHERE invocation_id > 'worker/' AND invocation_id < 'worker0' \
    AND (length(invocation_id) = 43 OR substr(invocation_id, 44, 1) = '/') \
    UNION SELECT DISTINCT substr(invocation_id, 8, 36) AS claim FROM recall_output \
    WHERE invocation_id > 'worker/' AND invocation_id < 'worker0' \
    AND (length(invocation_id) = 43 OR substr(invocation_id, 44, 1) = '/')), \
    claimed AS (SELECT DISTINCT event_id AS claim, run_id FROM recall_events \
    WHERE event_type = 'run_claimed' AND producer = 'scheduler'), \
    ranked AS (SELECT CAST(run_id AS TEXT) AS run_id, ended_at, ROW_NUMBER() OVER (PARTITION BY location_id, target \
    ORDER BY ended_at DESC, run_id DESC) AS newest FROM sqb_runs \
    WHERE ended_at IS NOT NULL AND status IN ('success', 'warn', 'partial', 'failed', 'cancelled')) \
    SELECT logged.claim FROM ranked JOIN claimed ON claimed.run_id = ranked.run_id \
    JOIN logged ON logged.claim = claimed.claim \
    WHERE ranked.newest > $1 OR ranked.ended_at < $2 ORDER BY ranked.ended_at LIMIT $3";

/// Returns how many audit rows outlive their horizon because a sink has not acknowledged them.
pub(crate) async fn retain(
    store: &ControlStore,
    RetentionPolicies {
        audit,
        statistics,
        snapshots,
        run_logs,
    }: RetentionPolicies,
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

/// A finished run's worker log goes whole once it is too old or too many newer runs finished; the run, its nodes and the scheduler's node history stay. Batches commit independently; a repeated or interrupted pass finds only logs still stored.
pub(crate) async fn prune_run_logs(
    store: &ControlStore,
    retention: RunLogRetention,
) -> Result<usize, ControlError> {
    let mut deleted = 0;
    loop {
        let transaction = begin(store, TransactionMode::Write).await?;
        let now = transaction.now;
        let (transaction, rows) = query(
            transaction,
            EXPIRED_CLAIMS,
            vec![
                Parameter::Integer(i64::from(retention.count())),
                Parameter::Time(retention.cutoff(now)),
                Parameter::Integer(RUN_LOG_RETENTION_BATCH as i64),
            ],
        )
        .await?;
        let claims: Vec<String> = rows
            .iter()
            .filter_map(|row| row["claim"].as_str().map(str::to_owned))
            .collect();
        let transaction = delete_logs(transaction, &claims).await?;
        commit(transaction).await?;
        deleted += claims.len();
        if claims.len() < RUN_LOG_RETENTION_BATCH {
            return Ok(deleted);
        }
    }
}

/// Every Recall row of the claims' invocations, then stored artifacts no remaining reference names. `worker/<claim>/<session>` sorts between `worker/<claim>/` and `worker/<claim>0` in byte order, which SQLite uses and PostgreSQL control stores declare for `invocation_id`, so each range reads the index.
async fn delete_logs(
    mut transaction: ControlTransaction,
    claims: &[String],
) -> Result<ControlTransaction, ControlError> {
    let parameters: Vec<Parameter> = claims
        .iter()
        .flat_map(|claim| {
            let namespace = format!("worker/{claim}");
            [format!("{namespace}/"), format!("{namespace}0"), namespace]
        })
        .map(Parameter::Text)
        .collect();
    let ranges: Vec<String> = (0..claims.len())
        .map(|index| {
            let first = 3 * index + 1;
            format!(
                "invocation_id=${} OR (invocation_id>${first} AND invocation_id<${})",
                first + 2,
                first + 1
            )
        })
        .collect();
    if ranges.is_empty() {
        return Ok(transaction);
    }
    let owned = format!("({})", ranges.join(" OR "));
    let rows;
    (transaction, rows) = query(
        transaction,
        &format!("SELECT DISTINCT digest FROM recall_artifact_refs WHERE {owned}"),
        parameters.clone(),
    )
    .await?;
    let digests: BTreeSet<String> = rows
        .iter()
        .filter_map(|row| row["digest"].as_str().map(str::to_owned))
        .collect();
    for table in INVOCATION_TABLES {
        (transaction, _) = query(
            transaction,
            &format!("DELETE FROM {} WHERE {owned}", table_spec(table).name),
            parameters.clone(),
        )
        .await?;
    }
    let digests: Vec<String> = digests.into_iter().collect();
    for chunk in digests.chunks(RUN_LOG_DIGEST_CHUNK) {
        (transaction, _) = query(
            transaction,
            &format!(
                "DELETE FROM recall_artifacts WHERE digest IN ({}) AND NOT EXISTS \
                 (SELECT 1 FROM recall_artifact_refs AS ref WHERE ref.digest = recall_artifacts.digest)",
                Parameter::placeholders(1..=chunk.len())
            ),
            chunk.iter().cloned().map(Parameter::Text).collect(),
        )
        .await?;
    }
    Ok(transaction)
}

/// Copy the WAL into the database and truncate it, or leave it for the next pass if readers stay.
pub(crate) async fn wal_checkpoint(store: &ControlStore) -> Result<(), ControlError> {
    let ControlStore::Sqlite(path) = store else {
        return Ok(());
    };
    let path = path.clone();
    tokio::task::spawn_blocking(move || {
        let connection = rusqlite::Connection::open(&path)?;
        connection.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)?;
        connection.busy_timeout(Duration::from_millis(CHECKPOINT_READER_WAIT_MS))?;
        connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        Ok(())
    })
    .await?
}
