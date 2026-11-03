//! Exchange consumes approval and issues a normal bounded person session atomically.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::_helpers::{
    active_session::active_session, browser_sessions::issue_session, secrets::secret_matches,
};
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, ClaimedLogin, StoredLogin};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn claim_login(
    store: &ControlStore,
    context: &AuditContext,
    (id, client_secret): (Uuid, &str),
) -> Result<ClaimedLogin, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, rows) = query(
        transaction,
        "SELECT client_hash,session_id FROM sqb_cli_login_flows WHERE flow_id=$1 AND expires_at>$2",
        vec![Parameter::Id(id), Parameter::Time(now)],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(ClaimedLogin::Unavailable);
    };
    let flow: StoredLogin = decode_row(row)?;
    if !secret_matches(client_secret, &flow.client_hash) {
        return Ok(ClaimedLogin::Unavailable);
    }
    let Some(parent) = flow.session_id else {
        return Ok(ClaimedLogin::Pending);
    };
    let (transaction, parent) = active_session(transaction, parent).await?;
    let Some(parent) = parent else {
        return Ok(ClaimedLogin::Unavailable);
    };
    let (transaction, _) = query(
        transaction,
        "DELETE FROM sqb_cli_login_flows WHERE flow_id=$1",
        vec![Parameter::Id(id)],
    )
    .await?;
    let (transaction, session) = issue_session(
        transaction,
        context,
        (parent.identity.principal_id, parent.credential_id),
    )
    .await?;
    commit(
        append_audit(
            transaction,
            context,
            (Uuid::new_v5(&id, b"exchanged"), "cli.login.exchanged"),
            (
                json!({"flow_id":id,"principal_id":session.identity.principal_id}),
                json!(null),
                json!({"session_id":session.identity.session_id}),
            ),
        )
        .await?,
    )
    .await?;
    Ok(ClaimedLogin::Authenticated(session))
}
