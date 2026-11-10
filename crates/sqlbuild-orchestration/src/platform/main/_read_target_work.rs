//! Read one request as a person reads it: only when it belongs to the location and target they named.

use uuid::Uuid;

use crate::platform::main::read_location_work::read_location_work;
use crate::platform::models::LocationWorkRecord;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(in crate::platform) async fn read_target_work(
    transaction: ControlTransaction,
    (location_id, target, request_id): (Uuid, &str, Uuid),
) -> Result<(ControlTransaction, Option<LocationWorkRecord>), ControlError> {
    let (transaction, stored) = read_location_work(transaction, request_id).await?;
    let record = stored.map(|stored| stored.record).filter(|record| {
        record.request_json.location_id == location_id && record.request_json.target == target
    });
    Ok((transaction, record))
}
