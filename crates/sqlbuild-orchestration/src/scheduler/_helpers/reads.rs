//! Tick history inside a caller's transaction, so authorization and results share one view.

use uuid::Uuid;

use crate::scheduler::models::TickRecord;
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn ticks_in(
    transaction: ControlTransaction,
    trigger_id: Uuid,
    limit: u32,
) -> Result<(ControlTransaction, Vec<TickRecord>), ControlError> {
    query_records(transaction,
        "SELECT tick_id, trigger_id, scheduled_for, evaluated_at, epoch, outcome, run_id, covered_by_run_id, active_run_id \
         FROM sqb_ticks WHERE trigger_id=$1 ORDER BY scheduled_for DESC LIMIT $2",
        vec![Parameter::Id(trigger_id), Parameter::Integer(i64::from(limit.min(1000)))]).await
}
