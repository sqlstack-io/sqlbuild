//! Skip one trigger that can't be evaluated now, so every other trigger keeps scheduling.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::platform::main::health::record_trigger_problem::record_trigger_problem;
use crate::platform::models::PlatformEvent;
use crate::scheduler::constants::{
    FAILED_SENSOR, FAILED_SENSOR_RETRY_SECONDS, UNREADABLE_TRIGGER,
    UNREADABLE_TRIGGER_RETRY_SECONDS,
};
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::{SchedulerLease, SkippedEpisode, SkippedTrigger};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Skip `trigger_id`, whose stored job or snapshot can't be read for `detail`; the server log line when its problem is new.
pub(crate) async fn skip_unreadable_trigger(
    store: &ControlStore,
    lease: &SchedulerLease,
    (trigger_id, detail): (Uuid, &str),
) -> Result<Option<String>, ControlError> {
    let episode = skip_trigger(
        store,
        lease,
        (trigger_id, UNREADABLE_TRIGGER_RETRY_SECONDS),
        UNREADABLE_TRIGGER.to_owned(),
    )
    .await?;
    Ok(episode.map(|episode| log_line(&episode, ("job or snapshot can't be read", detail))))
}

/// Skip sensor `trigger_id`, whose check failed with `error`, under a problem naming only the error's kind; the server log line, with the full error, when that problem is new.
pub(crate) async fn skip_failed_sensor(
    store: &ControlStore,
    lease: &SchedulerLease,
    trigger_id: Uuid,
    error: &ControlError,
) -> Result<Option<String>, ControlError> {
    let episode = skip_trigger(
        store,
        lease,
        (trigger_id, FAILED_SENSOR_RETRY_SECONDS),
        format!("{FAILED_SENSOR}: {}", error.kind_description()),
    )
    .await?;
    Ok(episode.map(|episode| log_line(&episode, ("check failed", &error.to_string()))))
}

/// Evaluate `trigger_id` again after `retry_seconds`, and record its `trigger_skipped` problem once per reason, snapshot and completed check; the episode when this call recorded it.
async fn skip_trigger(
    store: &ControlStore,
    lease: &SchedulerLease,
    (trigger_id, retry_seconds): (Uuid, i64),
    reason: String,
) -> Result<Option<SkippedEpisode>, ControlError> {
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
        "SELECT t.snapshot_id, t.kind, t.name, t.target, l.name AS location, \
         (SELECT MAX(e.occurred_at) FROM sqb_sensor_evaluation_events e \
         WHERE e.trigger_id=t.trigger_id AND e.phase='completed') AS checked_at \
         FROM sqb_triggers t JOIN sqb_locations l ON l.location_id=t.location_id WHERE t.trigger_id=$1",
        vec![Parameter::Id(trigger_id)],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        commit(check_lease(transaction, lease).await?).await?;
        return Ok(None);
    };
    let trigger: SkippedTrigger = decode_row(row)?;
    let (snapshot_id, checked_at) = (trigger.snapshot_id, trigger.checked_at);
    let (transaction, recorded) = record_trigger_problem(
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
    commit(check_lease(transaction, lease).await?).await?;
    Ok(recorded.map(|episode| SkippedEpisode { trigger, episode }))
}

/// One server log line with the full cause, findable from the health problem by its names and episode.
fn log_line(skipped: &SkippedEpisode, (what, cause): (&str, &str)) -> String {
    let trigger = &skipped.trigger;
    format!(
        "{} {} in location {}, target {}, was skipped: its {what} (problem episode {}): {cause}",
        trigger.kind, trigger.name, trigger.location, trigger.target, skipped.episode
    )
}
