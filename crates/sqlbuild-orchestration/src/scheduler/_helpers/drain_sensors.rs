//! The scheduler drains built-in sensors without importing project code.

use crate::scheduler::main::renew_lease::renew_lease;
use crate::scheduler::models::SchedulerLease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::main::complete_builtin_sensor::complete_builtin_sensor;
use crate::sensors::main::complete_custom_sensor::complete_custom_sensor;
use crate::sensors::models::SensorCondition;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn drain_sensors(
    store: &ControlStore,
    mut lease: SchedulerLease,
) -> Result<(SchedulerLease, usize), ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT t.trigger_id FROM sqb_triggers t JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id WHERE t.kind='sensor' AND s.status='running' \
         AND s.next_due_at<=$1 AND t.removed_in_snapshot_id IS NULL AND l.archived_at IS NULL \
         ORDER BY s.next_due_at,t.trigger_id LIMIT 100", vec![Parameter::Time(now)]).await?;
    commit(transaction).await?;
    let mut evaluated = 0;
    for row in rows {
        let trigger_id = serde_json::from_value(row["trigger_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        if let Some(request) = begin_sensor_evaluation(store, &lease, trigger_id).await? {
            if matches!(request.definition.when, SensorCondition::Custom { .. }) {
                complete_custom_sensor(store, &lease, &request).await?;
            } else {
                complete_builtin_sensor(store, &lease, &request).await?;
            }
            evaluated += 1;
        }
        lease = renew_lease(store, &lease).await?;
    }
    Ok((lease, evaluated))
}
