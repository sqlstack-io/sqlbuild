//! Drain a bounded page of due schedules or sensors, renewing the lease between persisted evaluations; a trigger whose job or snapshot can't be read is skipped on its own.

use crate::scheduler::_helpers::unreadable_trigger::skip_unreadable_trigger;
use crate::scheduler::main::_evaluate_trigger::evaluate_trigger;
use crate::scheduler::main::renew_lease::renew_lease;
use crate::scheduler::models::{DueKind, SchedulerLease};
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::main::complete_sensor::complete_sensor;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Evaluate up to 100 due triggers of `kind`, oldest due first; how many were evaluated and the ticks they took.
pub(crate) async fn drain_due(
    store: &ControlStore,
    (mut lease, origin): (SchedulerLease, Option<&str>),
    kind: DueKind,
) -> Result<(SchedulerLease, usize, usize), ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT s.trigger_id FROM sqb_trigger_state s JOIN sqb_triggers t ON t.trigger_id=s.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id \
         WHERE t.kind=$2 AND s.status='running' AND s.next_due_at<=$1 AND t.removed_in_snapshot_id IS NULL AND l.archived_at IS NULL \
         ORDER BY s.next_due_at, s.trigger_id LIMIT 100",
        vec![Parameter::Time(now), Parameter::Text(kind.stored().into())]).await?;
    commit(transaction).await?;
    let (mut evaluated, mut ticks) = (0, 0);
    for row in rows {
        let trigger_id: uuid::Uuid = serde_json::from_value(row["trigger_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        match evaluate_due(store, &lease, (trigger_id, origin), kind).await {
            Ok(Some(taken)) => {
                evaluated += 1;
                ticks += taken;
            }
            Ok(None) => {}
            Err(ControlError::UnreadableSnapshot { .. }) => {
                skip_unreadable_trigger(store, &lease, trigger_id).await?;
            }
            Err(error) => return Err(error),
        }
        lease = renew_lease(store, &lease).await?;
    }
    Ok((lease, evaluated, ticks))
}

/// Evaluate one due trigger; the ticks it took, or `None` when it was no longer due.
async fn evaluate_due(
    store: &ControlStore,
    lease: &SchedulerLease,
    (trigger_id, origin): (uuid::Uuid, Option<&str>),
    kind: DueKind,
) -> Result<Option<usize>, ControlError> {
    if kind == DueKind::Schedule {
        return Ok(evaluate_trigger(store, lease, trigger_id)
            .await?
            .map(|plan| plan.actions.len()));
    }
    let Some(request) = begin_sensor_evaluation(store, lease, (trigger_id, origin)).await? else {
        return Ok(None);
    };
    complete_sensor(store, lease, &request).await?;
    Ok(Some(0))
}
