//! A password hash becomes the person's credential under a new id, audited with the one it replaces.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlTransaction, Parameter};

/// Store `hash` as `principal_id`'s password; `before` is the replaced credential, or null.
pub(crate) async fn store_password(
    transaction: ControlTransaction,
    context: &AuditContext,
    (principal_id, hash): (Uuid, String),
    before: Value,
) -> Result<ControlTransaction, ControlError> {
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
    Ok(transaction)
}
