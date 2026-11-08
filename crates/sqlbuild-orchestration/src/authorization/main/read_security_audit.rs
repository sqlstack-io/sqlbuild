//! Live installation-wide audit permission and each bounded page share one transaction.

use uuid::Uuid;

use crate::authorization::_helpers::principal_names::principal_names;
use crate::authorization::main::begin_authorized::begin_authorized;
use crate::authorization::main::read_audit_events::read_audit_events;
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, PermissionScope, SecurityAuditEvent,
};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;

pub async fn read_security_audit(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (cursor, limit): ((i64, Option<i64>), u32),
) -> Result<Vec<SecurityAuditEvent>, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        (context, credentials),
        &PermissionScope::action(Action::AuditView, None),
        DefaultRole::None,
    )
    .await?;
    let (transaction, mut events) = read_audit_events(transaction, (cursor, limit)).await?;
    let (transaction, names) = principal_names(transaction).await?;
    commit(transaction).await?;
    for event in &mut events {
        event.actor_name = names.get(&event.actor).cloned();
    }
    Ok(events)
}
