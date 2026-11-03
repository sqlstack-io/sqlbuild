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
    let mut parameters = vec![
        Parameter::Id(session_id),
        Parameter::Id(principal_id),
        Parameter::Text(sha256_hex(bearer.as_bytes())),
        Parameter::Text(sha256_hex(csrf.as_bytes())),
        Parameter::Time(now),
        Parameter::Time(identity.expires_at),
    ];
    let (table, column, value) = match credential_id {
        Some(id) => {
            parameters.push(Parameter::Id(id));
            ("sqb_password_sessions", ",credential_id", ",$7")
        }
        None => ("sqb_external_sessions", "", ""),
    };
    let (transaction, _) = query(transaction,
        &format!("INSERT INTO {table} (session_id,principal_id,secret_hash,csrf_hash,created_at,expires_at{column}) VALUES ($1,$2,$3,$4,$5,$6{value})"),
        parameters).await?;
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
