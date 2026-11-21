//! One scheduler pass over due schedules and sensors, and the wait until the next one is due.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crate::scheduler::_helpers::drain_due::drain_due;
use crate::scheduler::models::{DueKind, SchedulerCycle, SchedulerLease};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, TransactionMode};

/// Schedules first, then sensors, each a bounded page.
pub(crate) async fn drain_due_triggers(
    store: &ControlStore,
    (lease, origin): (SchedulerLease, Option<&str>),
    mut cycle: SchedulerCycle,
) -> Result<(SchedulerLease, SchedulerCycle), ControlError> {
    let (lease, schedules, ticks) = drain_due(store, (lease, origin), DueKind::Schedule).await?;
    let (lease, sensors, _) = drain_due(store, (lease, origin), DueKind::Sensor).await?;
    cycle.evaluated += schedules + sensors;
    cycle.ticks += ticks;
    Ok((lease, cycle))
}

/// Shorten the wait to the next due time, never below zero, measured from the returned instant.
pub(crate) async fn wait_for_next_due(
    store: &ControlStore,
    wait_for: Duration,
) -> Result<(Duration, Instant), ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let now = transaction.now;
    let read_at = Instant::now();
    let (transaction, rows) = query(transaction,
        "SELECT MIN(s.next_due_at) AS next_due_at FROM sqb_trigger_state s JOIN sqb_triggers t ON t.trigger_id=s.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id \
         WHERE s.status='running' AND t.removed_in_snapshot_id IS NULL AND l.archived_at IS NULL", vec![]).await?;
    let next: Option<DateTime<Utc>> = serde_json::from_value(rows[0]["next_due_at"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    commit(transaction).await?;
    let wait_for = next.map_or(wait_for, |next| {
        (next - now)
            .to_std()
            .unwrap_or(Duration::ZERO)
            .min(wait_for)
    });
    Ok((wait_for, read_at))
}
