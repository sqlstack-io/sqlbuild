//! Skip one trigger that can't be evaluated now, so every other trigger keeps scheduling.

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use crate::platform::main::health::record_trigger_problem::record_trigger_problem;
use crate::platform::models::PlatformEvent;
use crate::scheduler::constants::{
    FAILED_SENSOR, FAILED_SENSOR_RETRY_SECONDS, UNREADABLE_TRIGGER,
    UNREADABLE_TRIGGER_RETRY_SECONDS,
};
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Skip `trigger_id`, whose stored job or snapshot can't be read, with a `trigger_skipped` health problem.
pub(crate) async fn skip_unreadable_trigger(
    store: &ControlStore,
    lease: &SchedulerLease,
    trigger_id: Uuid,
) -> Result<(), ControlError> {
    skip_trigger(
        store,
        lease,
        (trigger_id, UNREADABLE_TRIGGER_RETRY_SECONDS),
        UNREADABLE_TRIGGER.to_owned(),
    )
    .await
}

/// Skip sensor `trigger_id`, whose check failed with `error`, with a `trigger_skipped` health problem naming the error.
pub(crate) async fn skip_failed_sensor(
    store: &ControlStore,
    lease: &SchedulerLease,
    trigger_id: Uuid,
    error: &ControlError,
) -> Result<(), ControlError> {
    skip_trigger(
        store,
        lease,
        (trigger_id, FAILED_SENSOR_RETRY_SECONDS),
        format!("{FAILED_SENSOR}: {error}"),
    )
    .await
}

/// Evaluate `trigger_id` again after `retry_seconds`, and record its `trigger_skipped` problem once per reason, snapshot and completed check.
async fn skip_trigger(
    store: &ControlStore,
    lease: &SchedulerLease,
    (trigger_id, retry_seconds): (Uuid, i64),
    reason: String,
) -> Result<(), ControlError> {
    let transaction = check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
    let retry_at = transaction.now + TimeDelta::seconds(retry_seconds);
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_trigger_state SET next_due_at=$2 WHERE trigger_id=$1",
        vec![Parameter::Id(trigger_id), Parameter::Time(retry_at)],
    )
    .await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT t.snapshot_id, (SELECT MAX(e.occurred_at) FROM sqb_sensor_evaluation_events e \
         WHERE e.trigger_id=t.trigger_id AND e.phase='completed') AS checked_at \
         FROM sqb_triggers t WHERE t.trigger_id=$1",
        vec![Parameter::Id(trigger_id)],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return commit(check_lease(transaction, lease).await?).await;
    };
    let snapshot_id = serde_json::from_value::<Uuid>(row["snapshot_id"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let checked_at = serde_json::from_value::<Option<DateTime<Utc>>>(row["checked_at"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let transaction = record_trigger_problem(
        transaction,
        (
            PlatformEvent::TriggerSkipped {
                trigger_id,
                snapshot_id,
                reason,
            },
            checked_at,
        ),
    )
    .await?;
    commit(check_lease(transaction, lease).await?).await
}
