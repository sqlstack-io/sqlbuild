//! Token mutations compose within the caller's single audited write transaction.

use chrono::{DateTime, SubsecRound, Utc};
use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::credentials::_helpers::scopes::{scope_is_active, scope_parameters};
use crate::credentials::_helpers::secrets::new_bearer;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, CredentialScope, IssuedToken, TokenIdentity};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlTransaction, Parameter};

pub(crate) async fn issue(
    transaction: ControlTransaction,
    context: &AuditContext,
    scope: CredentialScope,
    expires_at: DateTime<Utc>,
) -> Result<(ControlTransaction, IssuedToken), ControlError> {
    let expires_at = expires_at.trunc_subsecs(6);
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
    let mut parameters = vec![
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
        "INSERT INTO sqb_access_tokens (token_id, secret_hash, location_id, run_id, scope_json, created_at, expires_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)", parameters).await?;
    let identity = TokenIdentity {
        token_id,
        scope,
        expires_at,
    };
    let transaction = append_audit(
        transaction,
        context,
        (Uuid::new_v5(&token_id, b"issued"), "token.issued"),
        (json!(scope), json!(null), json!(identity)),
    )
    .await?;
    Ok((transaction, IssuedToken { identity, bearer }))
}

pub(crate) async fn revoke(
    transaction: ControlTransaction,
    context: &AuditContext,
    token_id: Uuid,
) -> Result<(ControlTransaction, bool), ControlError> {
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "UPDATE sqb_access_tokens SET revoked_at=$1 WHERE token_id=$2 AND revoked_at IS NULL RETURNING scope_json",
        vec![Parameter::Time(now), Parameter::Id(token_id)]).await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok((transaction, false));
    };
    let scope = row
        .get("scope_json")
        .cloned()
        .ok_or_else(|| ControlError::Invalid("stored token scope is missing".into()))?;
    let mut transaction = append_audit(
        transaction,
        context,
        (Uuid::new_v5(&token_id, b"revoked"), "token.revoked"),
        (
            scope,
            json!({"token_id": token_id, "revoked": false}),
            json!({"token_id": token_id, "revoked": true}),
        ),
    )
    .await?;
    transaction.changes.insert(Change::Credentials);
    Ok((transaction, true))
}
