//! Deterministic tick insertion gates all run facts; retries cannot queue a second run.

use serde_json::json;

use crate::runs::main::job_write_set::job_write_set;
use crate::runs::main::queue_run::queue_run;
use crate::runs::models::{RunKind, RunOrigin, RunRequest};
use crate::scheduler::models::{DueTrigger, SchedulerLease, TickDecision, TickPlan};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn persist_ticks(
    transaction: ControlTransaction,
    (trigger, lease): (&DueTrigger, &SchedulerLease),
    plan: &TickPlan,
) -> Result<ControlTransaction, ControlError> {
    let now = transaction.now;
    let (mut transaction, write_set) = job_write_set(
        transaction,
        trigger.snapshot_id,
        &trigger.job_json.selection,
    )
    .await?;
    for action in &plan.actions {
        let (outcome, run, covered) = match action.decision {
            TickDecision::Requested { run_id } => ("requested", Some(run_id), None),
            TickDecision::Covered { covered_by_run_id } => {
                ("covered", None, Some(covered_by_run_id))
            }
        };
        let inserted;
        (transaction, inserted) = query(transaction,
            "INSERT INTO sqb_ticks (tick_id, trigger_id, scheduled_for, evaluated_at, epoch, outcome, run_id, covered_by_run_id, details_json) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (trigger_id, scheduled_for) DO NOTHING RETURNING tick_id",
            vec![Parameter::Id(action.tick_id), Parameter::Id(trigger.trigger_id), Parameter::Time(action.scheduled_for), Parameter::Time(now), Parameter::Integer(lease.epoch),
                Parameter::Text(outcome.into()), Parameter::OptionalId(run), Parameter::OptionalId(covered), Parameter::Json(json!({}))]).await?;
        if let Some(run_id) = run
            && !inserted.is_empty()
        {
            let request = RunRequest {
                run_id,
                location_id: trigger.location_id,
                target: trigger.target.clone(),
                snapshot_id: trigger.snapshot_id,
                image_id: trigger.image_id.clone(),
                kind: trigger.run_kind,
                origin: RunOrigin::Schedule,
                job_key: Some(trigger.job_key.clone()),
                parent_run_id: None,
                root_run_id: run_id,
                trigger_id: Some(trigger.trigger_id),
                tick_id: Some(action.tick_id),
                rerun_of_run_id: None,
                requested_by: "system:scheduler".into(),
                priority: 0,
                selection: trigger.job_json.selection.clone(),
                write_set: if trigger.run_kind == RunKind::Group {
                    Default::default()
                } else {
                    write_set.clone()
                },
                settings: trigger.job_json.settings.clone(),
                concurrency_groups: trigger.job_json.concurrency_groups.clone(),
                overlap: trigger.job_json.overlap,
            };
            transaction = queue_run(transaction, &request).await?;
        }
    }
    (transaction, _) = query(transaction,
        "UPDATE sqb_trigger_state SET last_tick_for=$1, next_due_at=$2, updated_at=$3 WHERE trigger_id=$4",
        vec![Parameter::OptionalTime(plan.last_tick_for), Parameter::OptionalTime(plan.next_due_at), Parameter::Time(now), Parameter::Id(trigger.trigger_id)]).await?;
    Ok(transaction)
}
