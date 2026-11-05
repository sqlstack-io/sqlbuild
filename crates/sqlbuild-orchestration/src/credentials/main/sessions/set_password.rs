//! Only active people have passwords; changing one rotates its identity and audits atomically.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::_helpers::passwords::hash_password;
use crate::credentials::constants::{MAX_PASSWORD_BYTES, MIN_PASSWORD_CHARACTERS};
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::main::sessions::revoke_session::revoke_session;
use crate::credentials::models::{AuditContext, PasswordAuthority};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Change, ControlStore, Parameter, TransactionMode};

/// The trusted caller authorizes password management before invoking this storage operation.
pub async fn set_password(
    store: &ControlStore,
    context: &AuditContext,
    authority: PasswordAuthority,
    password: &str,
) -> Result<(), ControlError> {
    if password.len() > MAX_PASSWORD_BYTES || password.chars().count() < MIN_PASSWORD_CHARACTERS {
        return Err(ControlError::Invalid(format!(
            "password must contain at least {MIN_PASSWORD_CHARACTERS} characters and at most {MAX_PASSWORD_BYTES} UTF-8 bytes"
        )));
    }
    let (principal_id, expected) = match authority {
        PasswordAuthority::Operator(id) => (id, None),
        PasswordAuthority::Reauthenticated { proof, session_id } => {
            (proof.principal_id, Some((proof.credential_id, session_id)))
        }
    };
    let hash = hash_password(password.into()).await?;
    let transaction = begin(store, TransactionMode::Write).await?;
    let (mut transaction, rows) = query(
        transaction,
        "SELECT c.credential_id FROM sqb_principals p \
         LEFT JOIN sqb_password_credentials c ON c.principal_id=p.principal_id \
         WHERE p.principal_id=$1 AND p.disabled_at IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    if rows.is_empty() {
        return Err(ControlError::Invalid(
            "password requires an active person".into(),
        ));
    }
    if let Some((id, session_id)) = expected {
        let now = transaction.now;
        let sessions;
        (transaction, sessions) = query(
            transaction,
            "SELECT session_id FROM sqb_password_sessions WHERE session_id=$1 AND principal_id=$2 \
             AND credential_id=$3 AND revoked_at IS NULL AND expires_at>$4",
            vec![
                Parameter::Id(session_id),
                Parameter::Id(principal_id),
                Parameter::Id(id),
                Parameter::Time(now),
            ],
        )
        .await?;
        if rows[0]["credential_id"] != json!(id) || sessions.is_empty() {
            return Err(ControlError::Invalid(
                "password credential or session changed; authenticate again".into(),
            ));
        }
    }
    let before = if rows[0]["credential_id"].is_null() {
        json!(null)
    } else {
        json!({"credential_id": rows[0]["credential_id"]})
    };
    let credential_id = Uuid::now_v7();
    let now = transaction.now;
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_password_credentials (principal_id, credential_id, password_hash, changed_at) \
         VALUES ($1,$2,$3,$4) ON CONFLICT (principal_id) DO UPDATE \
         SET credential_id=excluded.credential_id, password_hash=excluded.password_hash, changed_at=excluded.changed_at",
        vec![Parameter::Id(principal_id), Parameter::Id(credential_id), Parameter::Text(hash), Parameter::Time(now)],
    )
    .await?;
    let (transaction, _) = revoke_session(transaction, context, (principal_id, None)).await?;
    let mut transaction = append_audit(
        transaction,
        context,
        (credential_id, "password.changed"),
        (
            json!({"principal_id": principal_id}),
            before,
            json!({"credential_id": credential_id}),
        ),
    )
    .await?;
    transaction.changes.insert(Change::Credentials);
    commit(transaction).await
}
