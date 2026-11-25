//! A plan can't wait for a location that can't load the current code.

use uuid::Uuid;

use crate::platform::_helpers::health::recent_health::recent_health;
use crate::platform::_helpers::plans::load_failure::{load_failure_message, newest_load_error};
use crate::platform::models::LocationState;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// The error a plan request fails with while the target's newest load failed; `None` while it loads.
pub(crate) async fn load_failure_refusal(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, Option<String>), ControlError> {
    let now = transaction.now;
    let (transaction, observations) = recent_health(transaction, location_id, Some(target)).await?;
    let refusal = (LocationState::of(&observations, (target, now)) == LocationState::FallingBack)
        .then(|| load_failure_message(newest_load_error(&observations)));
    Ok((transaction, refusal))
}
