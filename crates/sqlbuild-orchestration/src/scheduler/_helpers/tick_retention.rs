//! Tick expiry is fenced, batched cleanup that keeps each trigger's cursor tick and run links.

use std::time::{Duration, Instant};

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use crate::scheduler::constants::{
    TICK_RETENTION_BATCH, TICK_RETENTION_BATCHES_PER_PASS, TICK_RETENTION_DAYS,
    TICK_RETENTION_INTERVAL_SECONDS,
};
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Change, ControlStore, Parameter, TransactionMode};

/// Evaluation never starts before the horizon, so a cursor reset cannot resurrect deleted ticks.
pub(crate) fn evaluation_start(
    (last_tick_for, started_at): (Option<DateTime<Utc>>, DateTime<Utc>),
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, ControlError> {
    let start = match last_tick_for {
        Some(last) => last,
        None => started_at
            .checked_sub_signed(TimeDelta::microseconds(1))
            .ok_or_else(|| ControlError::Invalid("trigger start is out of range".into()))?,
    };
    Ok(start.max(now - TimeDelta::days(TICK_RETENTION_DAYS)))
}

/// Run the leader's tick cleanup at most once per interval; a capped pass runs again next cycle.
pub(crate) async fn prune_ticks_when_due(
    store: &ControlStore,
    lease: &SchedulerLease,
    pruned_at: Option<Instant>,
) -> Result<Option<Instant>, ControlError> {
    let interval = Duration::from_secs(TICK_RETENTION_INTERVAL_SECONDS);
    match pruned_at {
        Some(at) if at.elapsed() < interval => Ok(Some(at)),
        _ => {
            let deleted = prune_ticks(store, lease).await?;
            Ok(
                if deleted == TICK_RETENTION_BATCH * TICK_RETENTION_BATCHES_PER_PASS {
                    pruned_at
                } else {
                    Some(Instant::now())
                },
            )
        }
    }
}

/// One bounded pass; requested, failed and deferred ticks stay, as does any tick a run points at.
pub(crate) async fn prune_ticks(
    store: &ControlStore,
    lease: &SchedulerLease,
) -> Result<usize, ControlError> {
    let mut deleted = 0;
    for _ in 0..TICK_RETENTION_BATCHES_PER_PASS {
        let mut transaction =
            check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
        let cutoff = transaction.now - TimeDelta::days(TICK_RETENTION_DAYS);
        let rows;
        (transaction, rows) = query(
            transaction,
            "DELETE FROM sqb_ticks WHERE tick_id IN (SELECT k.tick_id FROM sqb_ticks k \
             WHERE k.outcome IN ('skipped','covered') AND k.scheduled_for<$1 \
             AND k.scheduled_for<(SELECT MAX(l.scheduled_for) FROM sqb_ticks l WHERE l.trigger_id=k.trigger_id) \
             AND NOT EXISTS (SELECT 1 FROM sqb_runs r WHERE r.tick_id=k.tick_id) \
             ORDER BY k.scheduled_for, k.tick_id LIMIT $2) RETURNING trigger_id",
            vec![
                Parameter::Time(cutoff),
                Parameter::Integer(TICK_RETENTION_BATCH as i64),
            ],
        )
        .await?;
        for row in &rows {
            let trigger_id: Uuid = serde_json::from_value(row["trigger_id"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            transaction.changes.insert(Change::Trigger(trigger_id));
        }
        commit(check_lease(transaction, lease).await?).await?;
        deleted += rows.len();
        if rows.len() < TICK_RETENTION_BATCH {
            break;
        }
    }
    Ok(deleted)
}
