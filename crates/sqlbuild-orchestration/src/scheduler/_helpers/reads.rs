//! Tick history inside a caller's transaction, so authorization and results share one view.

use uuid::Uuid;

use crate::scheduler::models::TickRecord;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn ticks_in(
    transaction: ControlTransaction,
    trigger_id: Uuid,
    limit: u32,
) -> Result<(ControlTransaction, Vec<TickRecord>), ControlError> {
    query_records(transaction,
        "SELECT tick_id, trigger_id, scheduled_for, evaluated_at, epoch, outcome, run_id, covered_by_run_id \
         FROM sqb_ticks WHERE trigger_id=$1 ORDER BY scheduled_for DESC LIMIT $2",
        vec![Parameter::Id(trigger_id), Parameter::Integer(i64::from(limit.min(1000)))]).await
}

/// Whether `trigger_id` was ever declared in this location and target, including removed triggers.
pub(crate) async fn trigger_in_scope(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    trigger_id: Uuid,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT trigger_id FROM sqb_triggers WHERE trigger_id=$1 AND location_id=$2 AND target=$3",
        vec![
            Parameter::Id(trigger_id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    Ok((transaction, !rows.is_empty()))
}
