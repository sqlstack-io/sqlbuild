//! The scheduler records each trigger it skips as one health event per trigger and snapshot.

use uuid::Uuid;

use crate::platform::_helpers::health::observations::observe;
use crate::platform::models::PlatformEvent;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::ControlTransaction;

/// Record that `trigger_id`, published in `snapshot_id`, was skipped for `reason`; repeats for the same snapshot add nothing.
pub(crate) async fn record_trigger_skipped(
    transaction: ControlTransaction,
    (trigger_id, snapshot_id): (Uuid, Uuid),
    reason: &str,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT installation_id FROM sqb_installation WHERE singleton=1",
        vec![],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(transaction);
    };
    let installation_id = serde_json::from_value::<Uuid>(row["installation_id"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    observe(
        transaction,
        installation_id,
        (
            PlatformEvent::TriggerSkipped {
                trigger_id,
                snapshot_id,
                reason: reason.to_owned(),
            },
            None,
        ),
    )
    .await
}
