//! Operator reset returns the sensor to its initial cursor and forgets its delivered source events, so it reads everything again; the trigger change audit records it.

use uuid::Uuid;

use crate::sensors::_helpers::ticks::facts::commit_cursor;
use crate::sensors::models::SensorCursor;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn reset_sensor_cursor(
    transaction: ControlTransaction,
    trigger_id: Uuid,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, _) = query(
        transaction,
        "DELETE FROM sqb_sensor_deliveries WHERE trigger_id=$1",
        vec![Parameter::Id(trigger_id)],
    )
    .await?;
    commit_cursor(transaction, trigger_id, &SensorCursor::default()).await
}
