//! Disabling refuses every key of a service account at once; enabling accepts the same keys again.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::service_accounts::{
    audit_change, begin_account_change, finish_change,
};
use crate::credentials::models::{AuditContext, ServiceAccountAuthority, ServiceAccountSummary};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlStore, Parameter};

/// Disables or enables `principal_id`; a person needs `user.manage` everywhere. Repeating it changes nothing.
pub async fn set_service_account_disabled(
    store: &ControlStore,
    (context, authority): (&AuditContext, ServiceAccountAuthority),
    (principal_id, disabled): (Uuid, bool),
) -> Result<ServiceAccountSummary, ControlError> {
    let (transaction, authorized) =
        begin_account_change(store, (context, authority), principal_id).await?;
    let now = transaction.now;
    let (mut transaction, changed) = if disabled {
        query(transaction,
            "UPDATE sqb_principals SET disabled_at=$1 WHERE principal_id=$2 AND disabled_at IS NULL RETURNING principal_id",
            vec![Parameter::Time(now), Parameter::Id(principal_id)]).await?
    } else {
        query(transaction,
            "UPDATE sqb_principals SET disabled_at=NULL WHERE principal_id=$1 AND disabled_at IS NOT NULL RETURNING principal_id",
            vec![Parameter::Id(principal_id)]).await?
    };
    if !changed.is_empty() {
        transaction.changes.insert(Change::Credentials);
        transaction = audit_change(
            transaction,
            &authorized,
            (
                principal_id,
                if disabled {
                    "service_account.disabled"
                } else {
                    "service_account.enabled"
                },
            ),
            (
                json!({"disabled": !disabled}),
                json!({"disabled": disabled}),
            ),
        )
        .await?;
    }
    finish_change(transaction, principal_id).await
}
