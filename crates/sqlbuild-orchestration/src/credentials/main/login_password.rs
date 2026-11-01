//! Password login reserves shared throttle capacity before memory-hard verification.

use uuid::Uuid;

use crate::credentials::_helpers::password_sessions::issue_session;
use crate::credentials::constants::{LOGIN_USERNAME_BYTES, MAX_PASSWORD_BYTES};
use crate::credentials::main::record_authentication_denial::record_authentication_denial;
use crate::credentials::main::reserve_password_attempt::reserve_password_attempt;
use crate::credentials::main::verify_password::verify_password;
use crate::credentials::models::{AuditContext, PasswordAttempt, PasswordLogin};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn login_password(
    store: &ControlStore,
    context: &AuditContext,
    (username, password): (&str, &str),
) -> Result<PasswordLogin, ControlError> {
    let bounded = username.len() <= LOGIN_USERNAME_BYTES && password.len() <= MAX_PASSWORD_BYTES;
    let username = if bounded { username } else { "" };
    let (attempt_id, allowed) =
        reserve_password_attempt(store, context, PasswordAttempt::Login(username)).await?;
    if !allowed {
        return Ok(PasswordLogin::Denied {
            decision_id: attempt_id,
            throttled: true,
        });
    }
    if bounded {
        let transaction = begin(store, TransactionMode::Read).await?;
        let (transaction, rows) = query(
            transaction,
            "SELECT principal_id FROM sqb_principals WHERE name=$1",
            vec![Parameter::Text(username.into())],
        )
        .await?;
        commit(transaction).await?;
        let principal_id = rows
            .first()
            .map(|row| serde_json::from_value(row["principal_id"].clone()))
            .transpose()
            .map_err(|_| ControlError::Invalid("stored person identity is invalid".into()))?
            .unwrap_or(Uuid::nil());
        if let Some(proof) = verify_password(store, principal_id, password).await?
            && let Some(session) = issue_session(store, context, proof).await?
        {
            return Ok(PasswordLogin::Authenticated(session));
        }
    }
    let decision_id = record_authentication_denial(
        store,
        context,
        (Uuid::new_v5(&attempt_id, b"denied"), "invalid_credentials"),
        serde_json::json!({"attempt_id": attempt_id}),
    )
    .await?;
    Ok(PasswordLogin::Denied {
        decision_id,
        throttled: false,
    })
}
