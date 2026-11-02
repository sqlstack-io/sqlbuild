//! Live installation-wide audit permission and each bounded page share one transaction.

use crate::authorization::main::authorize_session::authorize_session;
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, PermissionScope, SecurityAuditEvent,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

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
    if after < 0 || !(1..=100).contains(&limit) {
        return Err(ControlError::Invalid(
            "audit pages require a nonnegative storage cursor and a limit between 1 and 100".into(),
        ));
    }
    let (transaction, rows) = query(transaction,
        "SELECT storage_id,event_id,occurred_at,actor,action,granting_role,request_id,ip_address,scope_json,before_json,after_json \
         FROM sqb_security_audit WHERE storage_id>$1 ORDER BY storage_id LIMIT $2",
        vec![Parameter::Integer(after), Parameter::Integer(i64::from(limit))]).await?;
    let events = rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    commit(transaction).await?;
    Ok(events)
}
