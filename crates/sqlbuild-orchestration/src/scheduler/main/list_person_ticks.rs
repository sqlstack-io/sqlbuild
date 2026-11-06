//! A person sees a trigger's ticks only with view on every current node; an unknown trigger is refused alike.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::scheduler::_helpers::reads::{ticks_in, trigger_in_scope};
use crate::scheduler::models::TickRecord;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// `None` when the trigger is not in this location and target; authorization runs first either way.
pub async fn list_person_ticks(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    ((location_id, target), (trigger_id, limit)): ((Uuid, &str), (Uuid, u32)),
    default_role: DefaultRole,
) -> Result<Option<Vec<TickRecord>>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, known) =
        trigger_in_scope(transaction, (location_id, target), trigger_id).await?;
    if !known {
        commit(transaction).await?;
        return Ok(None);
    }
    let (transaction, ticks) = ticks_in(transaction, trigger_id, limit).await?;
    commit(transaction).await?;
    Ok(Some(ticks))
}
