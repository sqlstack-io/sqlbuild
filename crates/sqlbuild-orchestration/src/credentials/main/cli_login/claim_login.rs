//! Exchange consumes approval and issues a bounded session while the approver stays live.

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
        "SELECT client_hash,principal_id,session_id FROM sqb_cli_login_flows WHERE flow_id=$1 AND expires_at>$2",
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
    let Some(principal_id) = flow.principal_id else {
        return Ok(ClaimedLogin::Pending);
    };
    let (transaction, approver) = match flow.session_id {
        Some(session_id) => active_session(transaction, session_id).await?,
        None => (transaction, None),
    };
    if flow.session_id.is_some() && approver.is_none() {
        return Ok(ClaimedLogin::Unavailable);
    }
    let (transaction, people) = query(
        transaction,
        "SELECT c.credential_id FROM sqb_principals p LEFT JOIN sqb_password_credentials c \
         ON c.principal_id=p.principal_id WHERE p.principal_id=$1 AND p.disabled_at IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    let Some(person) = people.first() else {
        return Ok(ClaimedLogin::Unavailable);
    };
    let credential_id = serde_json::from_value::<Option<Uuid>>(person["credential_id"].clone())
        .map_err(|error| ControlError::Schema(error.to_string()))?;
    let (transaction, _) = query(
        transaction,
        "DELETE FROM sqb_cli_login_flows WHERE flow_id=$1",
        vec![Parameter::Id(id)],
    )
    .await?;
    let (transaction, session) =
        issue_session(transaction, context, (principal_id, credential_id)).await?;
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
