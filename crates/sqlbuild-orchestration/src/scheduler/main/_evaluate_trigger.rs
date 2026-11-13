//! Fence, decide and publish using one serialized transaction and its database clock.

use uuid::Uuid;

use crate::declarations::models::TriggerStatus;
use crate::runs::main::active_job_run::active_job_run;
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
pub(crate) async fn evaluate_trigger(
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
    let (transaction, rows) = query(
        transaction,
        "SELECT r.run_id, r.request_json FROM sqb_runs r WHERE r.location_id=$1 AND r.target=$2 AND r.job_key=$3 \
         AND r.status='queued' AND r.cancel_requested_at IS NULL \
         AND r.worker_instance IS NULL ORDER BY r.queued_at, r.run_id",
        vec![
            Parameter::Id(trigger.location_id),
            Parameter::Text(trigger.target.clone()),
            Parameter::Text(trigger.job_key.clone()),
        ],
    )
    .await?;
    let covering: Vec<_> = rows
        .into_iter()
        .filter(|row| {
            row["request_json"]
                .get("retry")
                .is_none_or(serde_json::Value::is_null)
        })
        .take(1)
        .collect();
    let waiting_run = first_run(&covering)?;
    let (transaction, active_run) = if trigger.job_json.overlap.is_queue() {
        (transaction, None)
    } else {
        active_job_run(
            transaction,
            (trigger.location_id, &trigger.target, &trigger.job_key),
        )
        .await?
    };
    let plan = plan_ticks(
        &trigger.definition_json,
        &TickState {
            trigger_id,
            started_at: trigger.started_at,
            last_tick_for: trigger.last_tick_for,
            waiting_run,
            active_run,
        },
        now,
    )?;
    let transaction = persist_ticks(transaction, (&trigger, lease), &plan).await?;
    let transaction = check_lease(transaction, lease).await?;
    commit(transaction).await?;
    Ok(Some(plan))
}

fn first_run(rows: &[crate::store::types::Row]) -> Result<Option<Uuid>, ControlError> {
    rows.first()
        .and_then(|row| row.get("run_id"))
        .and_then(serde_json::Value::as_str)
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|error| ControlError::Invalid(error.to_string()))
}
