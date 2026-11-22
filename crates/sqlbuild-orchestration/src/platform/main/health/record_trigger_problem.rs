//! The scheduler and publication record a trigger's health problem as one event per payload.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::platform::_helpers::health::observations::observe_new;
use crate::platform::models::PlatformEvent;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::ControlTransaction;

/// Record `event`, a trigger's problem such as `PlatformEvent::TriggerSkipped`, and its ID when new; the same payload and `incident` again add nothing.
pub(crate) async fn record_trigger_problem(
    transaction: ControlTransaction,
    (event, incident): (PlatformEvent, Option<DateTime<Utc>>),
) -> Result<(ControlTransaction, Option<Uuid>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT installation_id FROM sqb_installation WHERE singleton=1",
        vec![],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok((transaction, None));
    };
    let installation_id = serde_json::from_value::<Uuid>(row["installation_id"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    observe_new(transaction, installation_id, (event, incident)).await
}
