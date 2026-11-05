//! Operator reset returns the sensor to its initial cursor; the trigger change audit records it.

use uuid::Uuid;

use crate::sensors::_helpers::facts::commit_cursor;
use crate::sensors::models::SensorCursor;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn reset_sensor_cursor(
    transaction: ControlTransaction,
    trigger_id: Uuid,
) -> Result<ControlTransaction, ControlError> {
    commit_cursor(transaction, trigger_id, &SensorCursor::default()).await
}
