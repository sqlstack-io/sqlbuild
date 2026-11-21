//! Any working key can be stopped at once, such as an old key before its overlap ends.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::service_accounts::{
    audit_change, begin_account_change, finish_change,
};
use crate::credentials::models::{AuditContext, ServiceAccountAuthority, ServiceAccountSummary};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlStore, Parameter};

/// Stops `key_id` of `principal_id` now; a person needs `user.manage` everywhere.
pub async fn revoke_service_key(
    store: &ControlStore,
    (context, authority): (&AuditContext, ServiceAccountAuthority),
    (principal_id, key_id): (Uuid, Uuid),
) -> Result<ServiceAccountSummary, ControlError> {
    let (transaction, authorized) =
        begin_account_change(store, (context, authority), principal_id).await?;
    let now = transaction.now;
    let (mut transaction, stopped) = query(
        transaction,
        "UPDATE sqb_service_account_keys SET revoked_at=$1 WHERE key_id=$2 AND principal_id=$3 \
         AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>$1) RETURNING prefix",
        vec![
            Parameter::Time(now),
            Parameter::Id(key_id),
            Parameter::Id(principal_id),
        ],
    )
    .await?;
    let Some(prefix) = stopped.first().and_then(|row| row["prefix"].as_str()) else {
        return Err(ControlError::Invalid(format!(
            "service account {principal_id} has no working key {key_id}"
        )));
    };
    let prefix = prefix.to_owned();
    transaction.changes.insert(Change::Credentials);
    let transaction = audit_change(
        transaction,
        &authorized,
        (principal_id, "service_account.key_revoked"),
        (
            json!({"key_id": key_id, "key_prefix": prefix, "revoked": false}),
            json!({"key_id": key_id, "key_prefix": prefix, "revoked": true}),
        ),
    )
    .await?;
    finish_change(transaction, principal_id).await
}
