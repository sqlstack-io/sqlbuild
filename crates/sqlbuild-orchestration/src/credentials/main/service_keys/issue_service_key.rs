//! A service account's key is made here, or imported by the owner, and stored only as its hash.

use uuid::Uuid;

use crate::credentials::_helpers::service_keys::{
    check_imported_key, key_hash, key_prefix, new_service_key,
};
use crate::credentials::models::{AuditContext, NewServiceKey, ServiceKeyOrigin};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlTransaction, Parameter};

/// Stores a new key for `principal_id`, or `chosen` once it passes the import rules, and returns it.
pub(crate) async fn issue_service_key(
    transaction: ControlTransaction,
    context: &AuditContext,
    (principal_id, chosen): (Uuid, Option<&str>),
) -> Result<(ControlTransaction, NewServiceKey), ControlError> {
    let (key, origin) = match chosen {
        Some(key) => {
            check_imported_key(key)?;
            (key.to_owned(), ServiceKeyOrigin::Imported)
        }
        None => (new_service_key()?, ServiceKeyOrigin::Issued),
    };
    let hash = key_hash(&key);
    let (transaction, taken) = query(
        transaction,
        "SELECT key_id FROM sqb_service_account_keys WHERE secret_hash=$1",
        vec![Parameter::Text(hash.clone())],
    )
    .await?;
    if !taken.is_empty() {
        return Err(ControlError::Invalid(
            "that key is already in use; choose another".into(),
        ));
    }
    let prefix = key_prefix(&key);
    let now = transaction.now;
    let (mut transaction, _) = query(
        transaction,
        "INSERT INTO sqb_service_account_keys (key_id, principal_id, secret_hash, prefix, origin, created_at, created_by) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
        vec![
            Parameter::Id(Uuid::now_v7()),
            Parameter::Id(principal_id),
            Parameter::Text(hash),
            Parameter::Text(prefix.clone()),
            Parameter::Text(origin.as_str().into()),
            Parameter::Time(now),
            Parameter::Text(context.actor.clone()),
        ],
    )
    .await?;
    transaction.changes.insert(Change::Credentials);
    Ok((
        transaction,
        NewServiceKey {
            key,
            prefix,
            origin,
        },
    ))
}
