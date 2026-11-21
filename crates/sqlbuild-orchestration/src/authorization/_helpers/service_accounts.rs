//! Service accounts: who may change them, how they read, and the keys a change writes.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::adder_scopes::{authorized, holds_all};
use crate::authorization::_helpers::principal_names::principal_names;
use crate::credentials::_helpers::service_keys::{key_hash, key_prefix};
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{
    Action, AuditContext, BuiltinRole, PermissionScope, ServiceAccountAuthority,
    ServiceAccountSummary, ServiceKeyOrigin, ServiceKeySummary,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, decode_row::decode_row, query::query};
use crate::store::models::{Change, ControlStore, ControlTransaction, Parameter, TransactionMode};

/// The longest service account name; names are also audit actors and CLI arguments.
const MAX_NAME_CHARACTERS: usize = 63;

/// Built-in roles from least to most, so an account holding several reports the highest.
const ROLE_ORDER: [BuiltinRole; 4] = [
    BuiltinRole::Viewer,
    BuiltinRole::Operator,
    BuiltinRole::Developer,
    BuiltinRole::Admin,
];

/// A write transaction for a change: authorized for every one of `scopes` for a person, or the owner's.
pub(crate) async fn begin_change(
    store: &ControlStore,
    (context, authority): (&AuditContext, ServiceAccountAuthority),
    scopes: &[PermissionScope],
) -> Result<(ControlTransaction, AuditContext), ControlError> {
    match authority {
        ServiceAccountAuthority::Owner => {
            Ok((begin(store, TransactionMode::Write).await?, context.clone()))
        }
        ServiceAccountAuthority::Person(actor) => authorized(store, (context, actor), scopes).await,
    }
}

/// A transaction for reading service accounts: `user.manage`, else `role.assign`, whose refusal is audited.
pub(crate) async fn begin_listing(
    store: &ControlStore,
    (context, authority): (&AuditContext, ServiceAccountAuthority),
) -> Result<ControlTransaction, ControlError> {
    let ServiceAccountAuthority::Person(actor) = authority else {
        return begin(store, TransactionMode::Read).await;
    };
    let manage = PermissionScope::action(Action::UserManage, None);
    let scope = if holds_all(store, actor, std::slice::from_ref(&manage)).await? {
        manage
    } else {
        PermissionScope::action(Action::RoleAssign, None)
    };
    Ok(authorized(store, (context, actor), &[scope]).await?.0)
}

/// Lowercase letters, digits, `-` and `_`, starting with a letter, 2 to 63 characters.
pub(crate) fn check_name(name: &str) -> Result<(), ControlError> {
    let valid = (2..=MAX_NAME_CHARACTERS).contains(&name.len())
        && name.starts_with(|first: char| first.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(ControlError::Invalid(format!(
            "a service account name is 2 to {MAX_NAME_CHARACTERS} lowercase letters, digits, - or _, \
             starting with a letter"
        )))
    }
}

/// The service account `principal_id`, refused as invalid when it is a person or doesn't exist.
pub(crate) async fn require_service_account(
    transaction: ControlTransaction,
    principal_id: Uuid,
) -> Result<(ControlTransaction, String), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT p.name FROM sqb_principals p JOIN sqb_service_accounts s ON s.principal_id=p.principal_id \
         WHERE p.principal_id=$1",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    let name = rows
        .first()
        .and_then(|row| row["name"].as_str())
        .ok_or_else(|| ControlError::Invalid(format!("no service account {principal_id}")))?;
    Ok((transaction, name.to_owned()))
}

/// The service account named `name`, for the CLI, which names accounts rather than numbering them.
pub(crate) async fn service_account_named(
    store: &ControlStore,
    name: &str,
) -> Result<Uuid, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await?,
        "SELECT p.principal_id FROM sqb_principals p JOIN sqb_service_accounts s ON s.principal_id=p.principal_id \
         WHERE p.name=$1",
        vec![Parameter::Text(name.into())],
    )
    .await?;
    crate::store::main::commit::commit(transaction).await?;
    rows.first()
        .and_then(|row| row["principal_id"].as_str())
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| ControlError::Invalid(format!("no service account named {name}")))
}

/// Stores a new key for `principal_id`, by its hash only, and returns its prefix.
pub(crate) async fn insert_key(
    transaction: ControlTransaction,
    context: &AuditContext,
    (principal_id, key, origin): (Uuid, &str, ServiceKeyOrigin),
) -> Result<(ControlTransaction, String), ControlError> {
    let hash = key_hash(key);
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
    let prefix = key_prefix(key);
    let now = transaction.now;
    let origin_text = match origin {
        ServiceKeyOrigin::Issued => "issued",
        ServiceKeyOrigin::Imported => "imported",
    };
    let (mut transaction, _) = query(
        transaction,
        "INSERT INTO sqb_service_account_keys (key_id, principal_id, secret_hash, prefix, origin, created_at, created_by) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
        vec![
            Parameter::Id(Uuid::now_v7()),
            Parameter::Id(principal_id),
            Parameter::Text(hash),
            Parameter::Text(prefix.clone()),
            Parameter::Text(origin_text.into()),
            Parameter::Time(now),
            Parameter::Text(context.actor.clone()),
        ],
    )
    .await?;
    transaction.changes.insert(Change::Credentials);
    Ok((transaction, prefix))
}

/// Records a change to service account `principal_id` in the audit log, under the acting principal.
pub(crate) async fn audit_change(
    transaction: ControlTransaction,
    context: &AuditContext,
    (principal_id, action): (Uuid, &str),
    (before, after): (serde_json::Value, serde_json::Value),
) -> Result<ControlTransaction, ControlError> {
    append_audit(
        transaction,
        context,
        (Uuid::now_v7(), action),
        (json!({"principal_id": principal_id}), before, after),
    )
    .await
}

#[derive(Deserialize)]
struct AccountRow {
    principal_id: Uuid,
    name: String,
    description: Option<String>,
    created_at: DateTime<Utc>,
    created_by: String,
    disabled_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct KeyRow {
    key_id: Uuid,
    principal_id: Uuid,
    prefix: String,
    origin: ServiceKeyOrigin,
    created_at: DateTime<Utc>,
    created_by: String,
    last_used_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

/// Service accounts by name, or only `principal_id`, with the keys that work at `transaction.now`.
pub(crate) async fn service_account_summaries(
    transaction: ControlTransaction,
    principal_id: Option<Uuid>,
) -> Result<(ControlTransaction, Vec<ServiceAccountSummary>), ControlError> {
    let now = transaction.now;
    let (transaction, accounts) = query(
        transaction,
        "SELECT p.principal_id, p.name, p.created_at, p.disabled_at, s.description, s.created_by \
         FROM sqb_principals p JOIN sqb_service_accounts s ON s.principal_id=p.principal_id \
         WHERE $1 IS NULL OR p.principal_id=$1 ORDER BY p.name",
        vec![Parameter::OptionalId(principal_id)],
    )
    .await?;
    let (transaction, keys) = query(transaction,
        "SELECT k.key_id, k.principal_id, k.prefix, k.origin, k.created_at, k.created_by, k.last_used_at, \
           k.expires_at, k.revoked_at FROM sqb_service_account_keys k \
         WHERE $1 IS NULL OR k.principal_id=$1 ORDER BY k.created_at DESC, k.key_id DESC",
        vec![Parameter::OptionalId(principal_id)]).await?;
    let (transaction, roles) = query(
        transaction,
        "SELECT a.principal_id, a.role_key FROM sqb_role_assignments a JOIN sqb_service_accounts s \
           ON s.principal_id=a.principal_id WHERE a.location_id IS NULL AND a.target IS NULL",
        Vec::new(),
    )
    .await?;
    let (transaction, names) = principal_names(transaction).await?;
    let made_by = |actor: &str| names.get(actor).map(|(name, _)| name.clone());
    let mut held: BTreeMap<Uuid, BuiltinRole> = BTreeMap::new();
    for row in roles {
        let (Some(id), Ok(role)) = (
            row["principal_id"]
                .as_str()
                .and_then(|id| Uuid::parse_str(id).ok()),
            serde_json::from_value::<BuiltinRole>(row["role_key"].clone()),
        ) else {
            continue;
        };
        let rank = |role: &BuiltinRole| ROLE_ORDER.iter().position(|r| r == role);
        if held
            .get(&id)
            .is_none_or(|current| rank(&role) > rank(current))
        {
            held.insert(id, role);
        }
    }
    let keys = keys
        .into_iter()
        .map(decode_row::<KeyRow>)
        .collect::<Result<Vec<_>, _>>()?;
    let summaries = accounts
        .into_iter()
        .map(|row| {
            let account: AccountRow = decode_row(row)?;
            let mine: Vec<&KeyRow> = keys
                .iter()
                .filter(|key| key.principal_id == account.principal_id)
                .collect();
            let last_used_at = mine.iter().filter_map(|key| key.last_used_at).max();
            let mut working: Vec<ServiceKeySummary> = mine
                .iter()
                .filter(|key| {
                    key.revoked_at.is_none() && key.expires_at.is_none_or(|end| end > now)
                })
                .map(|key| ServiceKeySummary {
                    key_id: key.key_id,
                    prefix: key.prefix.clone(),
                    origin: key.origin,
                    created_at: key.created_at,
                    created_by: made_by(&key.created_by),
                    last_used_at: key.last_used_at,
                    expires_at: key.expires_at,
                })
                .collect();
            working.sort_by_key(|key| key.expires_at.is_some());
            Ok(ServiceAccountSummary {
                principal_id: account.principal_id,
                role: held.get(&account.principal_id).copied(),
                created_by: made_by(&account.created_by),
                name: account.name,
                description: account.description,
                created_at: account.created_at,
                disabled_at: account.disabled_at,
                last_used_at,
                keys: working,
            })
        })
        .collect::<Result<Vec<_>, ControlError>>()?;
    Ok((transaction, summaries))
}

/// The one summary of `principal_id`, read after a change.
pub(crate) async fn service_account_summary(
    transaction: ControlTransaction,
    principal_id: Uuid,
) -> Result<(ControlTransaction, ServiceAccountSummary), ControlError> {
    let (transaction, mut summaries) =
        service_account_summaries(transaction, Some(principal_id)).await?;
    let summary = summaries
        .pop()
        .ok_or_else(|| ControlError::Invalid(format!("no service account {principal_id}")))?;
    Ok((transaction, summary))
}
