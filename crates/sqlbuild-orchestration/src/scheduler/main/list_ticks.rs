//! Tick history contains immutable evaluations, including covered schedule times.

use uuid::Uuid;

use crate::scheduler::models::TickRecord;
use crate::store::errors::ControlError;
use crate::store::main::read_records::read_records;
use crate::store::models::{ControlStore, Parameter};

pub async fn list_ticks(
    store: &ControlStore,
    trigger_id: Uuid,
    limit: u32,
) -> Result<Vec<TickRecord>, ControlError> {
    read_records(store,
        "SELECT tick_id, trigger_id, scheduled_for, evaluated_at, epoch, outcome, run_id, covered_by_run_id \
         FROM sqb_ticks WHERE trigger_id=$1 ORDER BY scheduled_for DESC LIMIT $2",
        vec![Parameter::Id(trigger_id), Parameter::Integer(i64::from(limit.min(1000)))]).await
}
