//! Every request sees revocation and current scope state, including across server replicas.

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::credentials::_helpers::scopes::scope_is_active;
use crate::credentials::constants::TOKEN_SECRET_HEX_LENGTH;
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
    let Some((id, secret)) = bearer.split_once('.') else {
        return Ok(None);
    };
    if secret.len() != TOKEN_SECRET_HEX_LENGTH
        || !secret.bytes().all(|value| value.is_ascii_hexdigit())
    {
        return Ok(None);
    }
    let Ok(id) = Uuid::parse_str(id) else {
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
    let digest = sha256_hex(bearer.as_bytes());
    if !bool::from(digest.as_bytes().ct_eq(token.secret_hash.as_bytes())) {
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
