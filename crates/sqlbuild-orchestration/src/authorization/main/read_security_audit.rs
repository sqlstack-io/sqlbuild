//! Live installation-wide audit permission and each bounded page share one transaction.

use uuid::Uuid;

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
    (after, limit): (i64, u32),
) -> Result<Vec<SecurityAuditEvent>, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        (context, credentials),
        &PermissionScope::action(Action::AuditView, None),
        DefaultRole::None,
    )
    .await?;
    let (transaction, events) = read_audit_events(transaction, (after, limit)).await?;
    commit(transaction).await?;
    Ok(events)
}
