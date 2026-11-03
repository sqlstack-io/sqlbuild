//! Every request sees revocation and current scope state, including across server replicas.

use crate::credentials::_helpers::scopes::scope_is_active;
use crate::credentials::_helpers::secrets::{bearer_id, secret_matches};
use crate::credentials::models::{StoredToken, TokenIdentity};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Invalid, revoked, expired and inactive-scope credentials are indistinguishable to callers.
pub async fn authenticate_token(
    store: &ControlStore,
    bearer: &str,
) -> Result<Option<TokenIdentity>, ControlError> {
    let Some(id) = bearer_id(bearer) else {
        return Ok(None);
    };
    let transaction = begin(store, TransactionMode::Read).await?;
    let now = transaction.now;
    let (transaction, rows) = query(
        transaction,
        "SELECT token_id, scope_json, secret_hash, expires_at FROM sqb_access_tokens \
         WHERE token_id=$1 AND revoked_at IS NULL AND expires_at>$2",
        vec![Parameter::Id(id), Parameter::Time(now)],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        commit(transaction).await?;
        return Ok(None);
    };
    let token: StoredToken = serde_json::from_value(serde_json::json!(row))
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    if !secret_matches(bearer, &token.secret_hash) {
        commit(transaction).await?;
        return Ok(None);
    }
    let (transaction, active) = scope_is_active(transaction, token.scope_json).await?;
    commit(transaction).await?;
    Ok(active.then_some(TokenIdentity {
        token_id: token.token_id,
        scope: token.scope_json,
        expires_at: token.expires_at,
    }))
}
