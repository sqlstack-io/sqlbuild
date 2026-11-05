//! Issue the same bounded, hashed browser credentials after the caller verifies a person.

use chrono::{Duration, SubsecRound};
use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::credentials::_helpers::secrets::new_bearer;
use crate::credentials::constants::PASSWORD_SESSION_SECONDS;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, IssuedSession, SessionIdentity};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// A live person gets a session; a password sign-in also needs its password version to be current.
pub(crate) async fn issue_live_session(
    transaction: ControlTransaction,
    context: &AuditContext,
    (principal_id, credential_id): (Uuid, Option<Uuid>),
    granting_role: &str,
) -> Result<(ControlTransaction, Option<(AuditContext, IssuedSession)>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT p.principal_id FROM sqb_principals p LEFT JOIN sqb_password_credentials c \
         ON c.principal_id=p.principal_id AND c.credential_id=$2 \
         WHERE p.principal_id=$1 AND p.disabled_at IS NULL AND ($2 IS NULL OR c.credential_id IS NOT NULL)",
        vec![Parameter::Id(principal_id), Parameter::OptionalId(credential_id)]).await?;
    if rows.is_empty() {
        return Ok((transaction, None));
    }
    let context = AuditContext {
        actor: format!("principal:{principal_id}"),
        granting_role: granting_role.into(),
        ..context.clone()
    };
    let (transaction, session) =
        issue_session(transaction, &context, (principal_id, credential_id)).await?;
    Ok((transaction, Some((context, session))))
}

pub(crate) async fn issue_session(
    transaction: ControlTransaction,
    context: &AuditContext,
    (principal_id, credential_id): (Uuid, Option<Uuid>),
) -> Result<(ControlTransaction, IssuedSession), ControlError> {
    let now = transaction.now;
    let session_id = Uuid::now_v7();
    let bearer = new_bearer(session_id)?;
    let csrf = new_bearer(session_id)?;
    let identity = SessionIdentity {
        session_id,
        principal_id,
        expires_at: (now + Duration::seconds(PASSWORD_SESSION_SECONDS)).trunc_subsecs(6),
    };
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_sessions (session_id,principal_id,secret_hash,csrf_hash,created_at,expires_at,credential_id) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        vec![
            Parameter::Id(session_id),
            Parameter::Id(principal_id),
            Parameter::Text(sha256_hex(bearer.as_bytes())),
            Parameter::Text(sha256_hex(csrf.as_bytes())),
            Parameter::Time(now),
            Parameter::Time(identity.expires_at),
            Parameter::OptionalId(credential_id),
        ]).await?;
    let transaction = append_audit(
        transaction,
        context,
        (session_id, "session.created"),
        (
            json!({"principal_id": principal_id}),
            json!(null),
            json!(&identity),
        ),
    )
    .await?;
    Ok((
        transaction,
        IssuedSession {
            identity,
            bearer,
            csrf,
        },
    ))
}
