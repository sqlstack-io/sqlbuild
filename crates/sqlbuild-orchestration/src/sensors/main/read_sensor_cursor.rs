//! Operator reads and evaluations share the cursor on the trigger state.

use uuid::Uuid;

use crate::sensors::models::SensorCursor;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_sensor_cursor(
    transaction: ControlTransaction,
    trigger_id: Uuid,
) -> Result<(ControlTransaction, SensorCursor), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT cursor_json FROM sqb_trigger_state WHERE trigger_id=$1 AND cursor_json IS NOT NULL",
        vec![Parameter::Id(trigger_id)],
    )
    .await?;
    let cursor = rows
        .first()
        .map(|row| serde_json::from_value(row["cursor_json"].clone()))
        .transpose()
        .map_err(|error| ControlError::Invalid(error.to_string()))?
        .unwrap_or_default();
    Ok((transaction, cursor))
}
