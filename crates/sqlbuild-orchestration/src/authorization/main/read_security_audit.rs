//! Live installation-wide audit permission and each bounded page share one transaction.

use crate::authorization::main::authorize_session::authorize_session;
use crate::authorization::main::read_audit_events::read_audit_events;
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, PermissionScope, SecurityAuditEvent,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn read_security_audit(
    store: &ControlStore,
    (context, credentials): (&AuditContext, (&str, &str)),
    (after, limit): (i64, u32),
) -> Result<Vec<SecurityAuditEvent>, ControlError> {
    let (transaction, _) = authorize_session(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        &PermissionScope {
            action: Action::AuditView,
            location_id: None,
            target: None,
            nodes: Default::default(),
        },
        DefaultRole::None,
    )
    .await?;
    let (transaction, events) = read_audit_events(transaction, (after, limit)).await?;
    commit(transaction).await?;
    Ok(events)
}
