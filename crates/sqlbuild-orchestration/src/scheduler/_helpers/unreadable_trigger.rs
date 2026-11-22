//! Skip one trigger that can't be evaluated now, so every other trigger keeps scheduling.

use chrono::{DateTime, TimeDelta, Utc};
use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;
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
        (UNREADABLE_TRIGGER, None),
    )
    .await
}

/// Skip sensor `trigger_id`, whose check failed with `error`, with a `trigger_skipped` health problem quoting the episode's first error.
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
        (FAILED_SENSOR, Some(error)),
    )
    .await
}

/// Evaluate `trigger_id` again after `retry_seconds`, and record its `trigger_skipped` problem once per `problem`, snapshot and completed check, keeping the first `error`.
async fn skip_trigger(
    store: &ControlStore,
    lease: &SchedulerLease,
    (trigger_id, retry_seconds): (Uuid, i64),
    (problem, error): (&str, Option<&ControlError>),
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
    let reason = error.map_or_else(
        || problem.to_owned(),
        |error| format!("{problem}: {}", redact_error_text(&error.to_string())),
    );
    let skipped = |reason: String| PlatformEvent::TriggerSkipped {
        trigger_id,
        snapshot_id,
        reason,
    };
    let transaction = record_trigger_problem(
        transaction,
        (
            skipped(reason),
            Some(skipped(problem.to_owned())),
            checked_at,
        ),
    )
    .await?;
    commit(check_lease(transaction, lease).await?).await
}
