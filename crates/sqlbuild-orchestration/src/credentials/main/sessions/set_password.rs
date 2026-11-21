//! Only active people have passwords; changing one rotates its identity and audits atomically.

use serde_json::json;

use crate::credentials::main::sessions::hash_new_password::hash_new_password;
use crate::credentials::main::sessions::revoke_session::revoke_session;
use crate::credentials::main::sessions::store_password::store_password;
use crate::credentials::models::{AuditContext, PasswordAuthority};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The trusted caller authorizes password management before invoking this storage operation.
pub async fn set_password(
    store: &ControlStore,
    context: &AuditContext,
    authority: PasswordAuthority,
    password: &str,
) -> Result<(), ControlError> {
    let (principal_id, expected) = match authority {
        PasswordAuthority::Operator(id) => (id, None),
        PasswordAuthority::Reauthenticated { proof, session_id } => {
            (proof.principal_id, Some((proof.credential_id, session_id)))
        }
    };
    let hash = hash_new_password(password).await?;
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
            "SELECT session_id FROM sqb_sessions WHERE session_id=$1 AND principal_id=$2 \
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
    let (transaction, _) = revoke_session(transaction, context, (principal_id, None)).await?;
    let transaction = store_password(transaction, context, (principal_id, hash), before).await?;
    commit(transaction).await
}
