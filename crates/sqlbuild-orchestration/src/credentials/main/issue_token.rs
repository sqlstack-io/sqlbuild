//! Random bearer secrets are returned once; only their hashes and bounded scopes are durable.

use chrono::{DateTime, SubsecRound, Utc};
use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::credentials::_helpers::scopes::{scope_is_active, scope_parameters};
use crate::credentials::_helpers::secrets::new_bearer;
use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::{AuditContext, CredentialScope, IssuedToken, TokenIdentity};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The caller authorizes token management before entering this storage operation.
pub async fn issue_token(
    store: &ControlStore,
    context: &AuditContext,
    scope: CredentialScope,
    expires_at: DateTime<Utc>,
) -> Result<IssuedToken, ControlError> {
    let expires_at = expires_at.trunc_subsecs(6);
    let transaction = begin(store, TransactionMode::Write).await?;
    if expires_at <= transaction.now {
        return Err(ControlError::Invalid(
            "token expiry must be in the future".into(),
        ));
    }
    let (transaction, active) = scope_is_active(transaction, scope).await?;
    if !active {
        return Err(ControlError::Invalid("token scope is not active".into()));
    }
    let token_id = Uuid::now_v7();
    let bearer = new_bearer(token_id)?;
    let now = transaction.now;
    let mut parameters: Vec<Parameter> = vec![
        Parameter::Id(token_id),
        Parameter::Text(sha256_hex(bearer.as_bytes())),
    ];
    parameters.extend(scope_parameters(scope));
    parameters.extend([
        Parameter::Json(json!(scope)),
        Parameter::Time(now),
        Parameter::Time(expires_at),
    ]);
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_access_tokens (token_id, secret_hash, principal_id, location_id, run_id, claim_id, scope_json, created_at, expires_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        parameters).await?;
    let identity = TokenIdentity {
        token_id,
        scope,
        expires_at,
    };
    commit(
        append_audit(
            transaction,
            context,
            (Uuid::new_v5(&token_id, b"issued"), "token.issued"),
            (json!(scope), json!(null), json!(identity)),
        )
        .await?,
    )
    .await?;
    Ok(IssuedToken { identity, bearer })
}
