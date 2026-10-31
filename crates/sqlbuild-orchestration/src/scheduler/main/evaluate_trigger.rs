//! Fence, decide and publish using one serialized transaction and its database clock.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::declarations::models::TriggerStatus;
use crate::scheduler::_helpers::expansion::expand_schedule;
use crate::scheduler::_helpers::persistence::persist_ticks;
use crate::scheduler::main::_plan_ticks::plan_ticks;
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::{DueTrigger, SchedulerLease, TickPlan, TickState};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// A removed or paused trigger is inert; a lost fence rolls back every write, including Recall.
pub async fn evaluate_trigger(
    store: &ControlStore,
    lease: &SchedulerLease,
    trigger_id: Uuid,
) -> Result<Option<TickPlan>, ControlError> {
    let transaction = check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
    let (transaction, rows) = query(transaction,
        "SELECT t.trigger_id, t.location_id, t.target, t.job_key, t.snapshot_id, s.image_id, t.definition_json, j.definition_json AS job_json, j.kind AS run_kind, \
         state.started_at, state.last_tick_for, state.status FROM sqb_triggers t \
         JOIN sqb_trigger_state state ON state.trigger_id=t.trigger_id JOIN sqb_snapshots s ON s.snapshot_id=t.snapshot_id \
         JOIN sqb_jobs j ON j.location_id=t.location_id AND j.target=t.target AND j.job_key=t.job_key \
         WHERE t.trigger_id=$1 AND t.kind='schedule' AND t.removed_in_snapshot_id IS NULL",
        vec![Parameter::Id(trigger_id)]).await?;
    let Some(trigger) = rows
        .into_iter()
        .next()
        .map(decode_row::<DueTrigger>)
        .transpose()?
    else {
        return Ok(None);
    };
    if trigger.status != TriggerStatus::Running {
        return Ok(None);
    }
    let now = transaction.now;
    let from = trigger.last_tick_for.unwrap_or(
        trigger
            .started_at
            .checked_sub_signed(TimeDelta::microseconds(1))
            .ok_or_else(|| ControlError::Invalid("trigger start is out of range".into()))?,
    );
    let (due_times, continuation) = expand_schedule(&trigger.definition_json, from, now)?;
    let (transaction, rows) = query(transaction,
        "SELECT r.run_id FROM sqb_runs r WHERE r.location_id=$1 AND r.target=$2 AND r.job_key=$3 \
         AND r.status='queued' AND r.cancel_requested_at IS NULL \
         AND NOT EXISTS (SELECT 1 FROM sqb_run_liveness l WHERE l.run_id=r.run_id) ORDER BY r.queued_at, r.run_id LIMIT 1",
        vec![Parameter::Id(trigger.location_id), Parameter::Text(trigger.target.clone()), Parameter::Text(trigger.job_key.clone())]).await?;
    let waiting_run = rows
        .first()
        .and_then(|row| row.get("run_id"))
        .and_then(serde_json::Value::as_str)
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let plan = plan_ticks(
        &TickState {
            trigger_id,
            enabled: true,
            last_tick_for: trigger.last_tick_for,
            due_times,
            continuation,
            catch_up: trigger.definition_json.catch_up,
            overlap: trigger.job_json.overlap,
            waiting_run,
        },
        now,
    )?;
    let transaction = persist_ticks(transaction, (&trigger, lease), &plan).await?;
    let transaction = check_lease(transaction, lease).await?;
    commit(transaction).await?;
    Ok(Some(plan))
}
