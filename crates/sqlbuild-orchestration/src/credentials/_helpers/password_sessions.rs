//! Session creation rechecks the verified password version under the control-store write lock.

use chrono::{Duration, SubsecRound};
use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::credentials::_helpers::secrets::new_bearer;
use crate::credentials::constants::PASSWORD_SESSION_SECONDS;
use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::{AuditContext, IssuedSession, SessionIdentity, VerifiedPassword};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn issue_session(
    store: &ControlStore,
    context: &AuditContext,
    proof: VerifiedPassword,
) -> Result<Option<IssuedSession>, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT p.principal_id FROM sqb_principals p JOIN sqb_password_credentials c ON p.principal_id=c.principal_id \
         WHERE p.principal_id=$1 AND c.credential_id=$2 AND p.kind='person' AND p.disabled_at IS NULL",
        vec![Parameter::Id(proof.principal_id), Parameter::Id(proof.credential_id)]).await?;
    if rows.is_empty() {
        commit(transaction).await?;
        return Ok(None);
    }
    let session_id = Uuid::now_v7();
    let bearer = new_bearer(session_id)?;
    let csrf = new_bearer(session_id)?;
    let identity = SessionIdentity {
        session_id,
        principal_id: proof.principal_id,
        expires_at: (now + Duration::seconds(PASSWORD_SESSION_SECONDS)).trunc_subsecs(6),
    };
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_password_sessions (session_id, principal_id, credential_id, secret_hash, csrf_hash, created_at, expires_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
        vec![Parameter::Id(session_id), Parameter::Id(proof.principal_id), Parameter::Id(proof.credential_id),
             Parameter::Text(sha256_hex(bearer.as_bytes())), Parameter::Text(sha256_hex(csrf.as_bytes())),
             Parameter::Time(now), Parameter::Time(identity.expires_at)]).await?;
    let context = AuditContext {
        actor: format!("principal:{}", proof.principal_id),
        granting_role: "password-session".into(),
        ..context.clone()
    };
    commit(
        append_audit(
            transaction,
            &context,
            (session_id, "session.created"),
            (
                json!({"principal_id": proof.principal_id}),
                json!(null),
                json!(&identity),
            ),
        )
        .await?,
    )
    .await?;
    Ok(Some(IssuedSession {
        identity,
        bearer,
        csrf,
    }))
}
