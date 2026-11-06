//! Whether a run or trigger, including a removed trigger, belongs to one location and target.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter, TargetRow};

pub(crate) async fn row_in_target(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    row: TargetRow,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (sql, id) = match row {
        TargetRow::Run(run_id) => (
            "SELECT run_id FROM sqb_runs WHERE run_id=$1 AND location_id=$2 AND target=$3",
            run_id,
        ),
        TargetRow::Trigger(trigger_id) => (
            "SELECT trigger_id FROM sqb_triggers WHERE trigger_id=$1 AND location_id=$2 AND target=$3",
            trigger_id,
        ),
    };
    let (transaction, rows) = query(
        transaction,
        sql,
        vec![
            Parameter::Id(id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    Ok((transaction, !rows.is_empty()))
}
