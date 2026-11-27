//! A stored event's recorded skip, for dismissing its problem.

use crate::platform::_helpers::health::skipped_events::{dismissed_skips, recorded_skips};
use crate::platform::models::SkippedEvent;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// The skip recorded for stored event `storage_id`, if the scan still reads one, and whether someone dismissed it.
pub(crate) async fn recorded_skip(
    transaction: ControlTransaction,
    storage_id: i64,
) -> Result<(ControlTransaction, Option<SkippedEvent>, bool), ControlError> {
    let (transaction, recorded) = recorded_skips(transaction).await?;
    let (transaction, dismissed) = dismissed_skips(transaction).await?;
    let skip = recorded
        .into_iter()
        .find(|event| event.storage_id == storage_id);
    Ok((transaction, skip, dismissed.contains(&storage_id)))
}
