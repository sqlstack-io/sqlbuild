//! Session creation rechecks the verified password version under the control-store write lock.

use crate::credentials::_helpers::browser_sessions;
use crate::credentials::models::{AuditContext, IssuedSession, VerifiedPassword};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn issue_session(
    store: &ControlStore,
    context: &AuditContext,
    proof: VerifiedPassword,
) -> Result<Option<IssuedSession>, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, rows) = query(transaction,
        "SELECT p.principal_id FROM sqb_principals p JOIN sqb_password_credentials c ON p.principal_id=c.principal_id \
         WHERE p.principal_id=$1 AND c.credential_id=$2 AND p.disabled_at IS NULL",
        vec![Parameter::Id(proof.principal_id), Parameter::Id(proof.credential_id)]).await?;
    if rows.is_empty() {
        commit(transaction).await?;
        return Ok(None);
    }
    let context = AuditContext {
        actor: format!("principal:{}", proof.principal_id),
        granting_role: "password-session".into(),
        ..context.clone()
    };
    let (transaction, session) = browser_sessions::issue_session(
        transaction,
        &context,
        (proof.principal_id, Some(proof.credential_id)),
    )
    .await?;
    commit(transaction).await?;
    Ok(Some(session))
}
