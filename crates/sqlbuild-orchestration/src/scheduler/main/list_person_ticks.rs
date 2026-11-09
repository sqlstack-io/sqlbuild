//! A person sees a trigger's ticks only with view on every current node; an unknown trigger is refused alike.

use uuid::Uuid;

use crate::authorization::main::row_in_target::row_in_target;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::scheduler::_helpers::reads::ticks_in;
use crate::scheduler::models::TickRecord;
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TargetRow};

/// `None` when the trigger is not in this location and target; authorization runs first either way.
pub async fn list_person_ticks(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    ((location_id, target), (trigger_id, limit)): ((Uuid, &str), (Uuid, u32)),
    default_role: DefaultRole,
) -> Result<Option<Vec<TickRecord>>, ControlError> {
    let Some(transaction) = row_in_target(
        store,
        (context, credentials),
        ((location_id, target), TargetRow::Trigger(trigger_id)),
        default_role,
    )
    .await?
    else {
        return Ok(None);
    };
    let (transaction, ticks) = ticks_in(transaction, trigger_id, limit).await?;
    commit(transaction).await?;
    Ok(Some(ticks))
}
