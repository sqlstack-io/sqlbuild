//! Refuse work under a claim that is no longer the run's active claim.

use uuid::Uuid;

use crate::runs::_helpers::lifecycle::output::claim_envelope;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// An error unless `claim_id` is `run_id`'s active claim.
pub(crate) async fn require_active_claim(
    transaction: ControlTransaction,
    claim: (Uuid, Uuid),
) -> Result<ControlTransaction, ControlError> {
    let (transaction, _) = claim_envelope(transaction, claim).await?;
    Ok(transaction)
}
