//! An exclusively owned offline transaction restores a person, never reopens bootstrap.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::administrators::grant_installation_admin;
use crate::authorization::main::deny_authorization::deny;
use crate::credentials::main::append_audit::append_audit;
use crate::credentials::main::revoke_session::revoke_session;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{Change, OfflineRecovery, Parameter};

pub async fn recover_administrator(
    OfflineRecovery(transaction): OfflineRecovery,
    context: &AuditContext,
    (principal_id, reason): (Uuid, &str),
) -> Result<(), ControlError> {
    if reason.trim().is_empty() {
        return Err(ControlError::Invalid(
            "offline recovery requires a reason".into(),
        ));
    }
    let (transaction, initialized) = query(
        transaction,
        "SELECT principal_id FROM sqb_admin_initialization WHERE singleton=1",
        vec![],
    )
    .await?;
    if initialized.is_empty() {
        return deny(
            transaction,
            context,
            json!({"principal_id":principal_id}),
            "administrator initialization has not occurred; use create-user",
        )
        .await;
    }
    let (transaction, people) = query(
        transaction,
        "SELECT disabled_at FROM sqb_principals WHERE principal_id=$1 AND kind='person'",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    let Some(person) = people.first() else {
        return deny(
            transaction,
            context,
            json!({"principal_id":principal_id}),
            "offline recovery requires an existing person",
        )
        .await;
    };
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_principals SET disabled_at=NULL WHERE principal_id=$1",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    let (transaction, _) =
        grant_installation_admin(transaction, context, principal_id, "offline_recovery").await?;
    let (mut transaction, sessions_revoked) =
        revoke_session(transaction, context, (principal_id, None)).await?;
    transaction = append_audit(transaction, context, (Uuid::now_v7(), "admin.offline_recovered"),
        (json!({"principal_id":principal_id}),json!({"enabled":person["disabled_at"].is_null()}),
         json!({"enabled":true,"role":"admin","reason":reason,"sessions_revoked":sessions_revoked}))).await?;
    transaction.changes.insert(Change::Credentials);
    commit(transaction).await
}
