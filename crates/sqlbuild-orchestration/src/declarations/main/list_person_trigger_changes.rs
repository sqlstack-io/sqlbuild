//! A person sees a trigger's on/off history only with view on every current node; an unknown trigger is refused alike.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::authorization::main::row_in_target::row_in_target;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::declarations::_helpers::reads::trigger_changes_in;
use crate::declarations::models::PersonTriggerChange;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TargetRow, TransactionMode};

/// `None` when the trigger is not in this location and target; authorization runs first either way.
pub async fn list_person_trigger_changes(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    ((location_id, target), (trigger_id, limit)): ((Uuid, &str), (Uuid, u32)),
    default_role: DefaultRole,
) -> Result<Option<Vec<PersonTriggerChange>>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, known) = row_in_target(
        transaction,
        (location_id, target),
        TargetRow::Trigger(trigger_id),
    )
    .await?;
    if !known {
        commit(transaction).await?;
        return Ok(None);
    }
    let (transaction, changes) = trigger_changes_in(transaction, trigger_id, limit).await?;
    commit(transaction).await?;
    Ok(Some(changes))
}
