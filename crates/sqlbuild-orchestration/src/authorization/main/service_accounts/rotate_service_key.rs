//! Rotation issues a new key; the replaced key works on for the overlap, and at most one old key is kept.

use chrono::Duration;
use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::service_accounts::{
    audit_change, begin_change, insert_key, require_service_account, service_account_summary,
};
use crate::credentials::_helpers::service_keys::new_service_key;
use crate::credentials::constants::MAX_KEY_OVERLAP_MINUTES;
use crate::credentials::models::{
    Action, AuditContext, IssuedServiceKey, PermissionScope, ServiceAccountAuthority,
    ServiceKeyOrigin, ServiceKeyRotation,
};
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter};

/// Issues `principal_id` a new key; a person needs `user.manage` everywhere.
///
/// A key already in an overlap stops now. The current key stops now too, unless `rotation` gives it an
/// overlap. A disabled account can be rotated, so a leaked key can be disabled, rotated and enabled.
pub async fn rotate_service_key(
    store: &ControlStore,
    (context, authority): (&AuditContext, ServiceAccountAuthority),
    (principal_id, rotation): (Uuid, ServiceKeyRotation),
) -> Result<IssuedServiceKey, ControlError> {
    let overlap = rotation.overlap_minutes.unwrap_or(0);
    if !(0..=MAX_KEY_OVERLAP_MINUTES).contains(&overlap) {
        return Err(ControlError::Invalid(format!(
            "the overlap is 0 to {MAX_KEY_OVERLAP_MINUTES} minutes"
        )));
    }
    let (transaction, authorized) = begin_change(
        store,
        (context, authority),
        &[PermissionScope::action(Action::UserManage, None)],
    )
    .await?;
    let (transaction, _) = require_service_account(transaction, principal_id).await?;
    let now = transaction.now;
    let (transaction, replaced) = query(transaction,
        "UPDATE sqb_service_account_keys SET revoked_at=$1 WHERE principal_id=$2 AND revoked_at IS NULL \
         AND expires_at IS NOT NULL AND expires_at>$1 RETURNING prefix",
        vec![Parameter::Time(now), Parameter::Id(principal_id)]).await?;
    let ends = (overlap > 0).then(|| now + Duration::minutes(overlap));
    let (transaction, current) = match ends {
        Some(ends) => query(transaction,
            "UPDATE sqb_service_account_keys SET expires_at=$1 WHERE principal_id=$2 AND revoked_at IS NULL \
             AND expires_at IS NULL RETURNING prefix",
            vec![Parameter::Time(ends), Parameter::Id(principal_id)]).await?,
        None => query(transaction,
            "UPDATE sqb_service_account_keys SET revoked_at=$1 WHERE principal_id=$2 AND revoked_at IS NULL \
             AND expires_at IS NULL RETURNING prefix",
            vec![Parameter::Time(now), Parameter::Id(principal_id)]).await?,
    };
    let key = new_service_key()?;
    let (transaction, prefix) = insert_key(
        transaction,
        &authorized,
        (principal_id, &key, ServiceKeyOrigin::Issued),
    )
    .await?;
    let prefixes = |rows: &[crate::store::types::Row]| -> Vec<String> {
        rows.iter()
            .filter_map(|row| row["prefix"].as_str().map(str::to_owned))
            .collect()
    };
    let transaction = audit_change(
        transaction,
        &authorized,
        (principal_id, "service_account.key_rotated"),
        (
            json!({"key_prefixes": prefixes(&current), "stopped_old_key_prefixes": prefixes(&replaced)}),
            json!({"key_prefix": prefix, "old_key_expires_at": ends}),
        ),
    )
    .await?;
    let (transaction, account) = service_account_summary(transaction, principal_id).await?;
    commit(transaction).await?;
    Ok(IssuedServiceKey { account, key })
}
