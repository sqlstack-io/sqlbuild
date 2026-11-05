//! A password proves one credential version; it never directly grants a role or an API token.

use uuid::Uuid;

use crate::credentials::_helpers::passwords::verify_hash;
use crate::credentials::constants::{DUMMY_PASSWORD_HASH, MAX_PASSWORD_BYTES};
use crate::credentials::models::VerifiedPassword;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Network callers must apply their shared login throttle before invoking this primitive.
pub async fn verify_password(
    store: &ControlStore,
    principal_id: Uuid,
    password: &str,
) -> Result<Option<VerifiedPassword>, ControlError> {
    if password.len() > MAX_PASSWORD_BYTES {
        return Ok(None);
    }
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT credential_id, password_hash FROM sqb_password_credentials c \
         JOIN sqb_principals p ON p.principal_id=c.principal_id \
         WHERE p.principal_id=$1 AND p.disabled_at IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    commit(transaction).await?;
    let row = rows.into_iter().next();
    let (identity, hash) = if let Some(row) = row {
        let credential_id = serde_json::from_value(row["credential_id"].clone())
            .map_err(|_| ControlError::Invalid("stored password identity is invalid".into()))?;
        let hash = serde_json::from_value(row["password_hash"].clone())
            .map_err(|_| ControlError::Invalid("stored password hash is invalid".into()))?;
        (
            Some(VerifiedPassword {
                principal_id,
                credential_id,
            }),
            hash,
        )
    } else {
        (None, DUMMY_PASSWORD_HASH.into())
    };
    Ok(verify_hash(hash, password.into())
        .await?
        .then_some(identity)
        .flatten())
}
