//! Serialized reservations bound authentication work across API replicas.

use chrono::Duration;
use uuid::Uuid;

use crate::credentials::constants::{LOGIN_PEER_LIMIT, LOGIN_USER_LIMIT, LOGIN_WINDOW_SECONDS};
use crate::credentials::models::{AuditContext, AuthenticationAttempt};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn reserve_authentication_attempt(
    store: &ControlStore,
    context: &AuditContext,
    attempt: AuthenticationAttempt<'_>,
) -> Result<(Uuid, bool), ControlError> {
    let peer = context
        .ip_address
        .ok_or_else(|| ControlError::Invalid("login requires a trusted peer address".into()))?
        .to_canonical()
        .to_string();
    let (identity_key, identity_limit) = match attempt {
        AuthenticationAttempt::Login(username) => (format!("login:{username}"), LOGIN_USER_LIMIT),
        AuthenticationAttempt::Reauthenticate(id) => {
            (format!("reauthenticate:{id}"), LOGIN_USER_LIMIT)
        }
        AuthenticationAttempt::IdentityProvider => {
            (format!("identity-provider:{peer}"), LOGIN_PEER_LIMIT)
        }
    };
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "DELETE FROM sqb_password_login_attempts WHERE attempt_id IN \
         (SELECT attempt_id FROM sqb_password_login_attempts WHERE attempted_at<$1 ORDER BY attempted_at LIMIT 1000)",
        vec![Parameter::Time(now - Duration::hours(1))]).await?;
    let (mut transaction, rows) = query(transaction,
        "SELECT (SELECT COUNT(*) FROM sqb_password_login_attempts WHERE identity_key=$1 AND attempted_at>$3) AS user_count, \
         (SELECT COUNT(*) FROM sqb_password_login_attempts WHERE peer_address=$2 AND attempted_at>$3) AS peer_count",
        vec![Parameter::Text(identity_key.clone()), Parameter::Text(peer.clone()), Parameter::Time(now - Duration::seconds(LOGIN_WINDOW_SECONDS))]).await?;
    let allowed = rows[0]["user_count"]
        .as_u64()
        .is_some_and(|count| count < identity_limit)
        && rows[0]["peer_count"]
            .as_u64()
            .is_some_and(|count| count < LOGIN_PEER_LIMIT);
    let id = Uuid::now_v7();
    if allowed {
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_password_login_attempts (attempt_id, identity_key, peer_address, attempted_at) VALUES ($1,$2,$3,$4)",
            vec![Parameter::Id(id), Parameter::Text(identity_key.clone()), Parameter::Text(peer), Parameter::Time(now)]).await?;
    }
    commit(transaction).await?;
    Ok((id, allowed))
}
