//! Service accounts: who may change them, how they are added, and how they read.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::adder_scopes::{authorized, holds_all};
use crate::authorization::_helpers::principal_names::principal_names;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::main::identities::insert_principal::insert_principal;
use crate::credentials::models::{
    Action, AuditContext, BuiltinRole, PermissionScope, PrincipalKind, ServiceAccountAuthority,
    ServiceAccountSummary, ServiceKeyOrigin, ServiceKeySummary,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{Change, ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::store::types::Row;

/// The longest service account name; names are also audit actors and CLI arguments.
const MAX_NAME_CHARACTERS: usize = 63;

/// Built-in roles from least to most, so an account holding several reports the highest.
const ROLE_ORDER: [BuiltinRole; 4] = [
    BuiltinRole::Viewer,
    BuiltinRole::Operator,
    BuiltinRole::Developer,
    BuiltinRole::Admin,
];

type Names = BTreeMap<String, (String, PrincipalKind)>;

#[derive(Deserialize)]
struct PrincipalRow {
    principal_id: Uuid,
}

#[derive(Deserialize)]
struct RoleRow {
    principal_id: Uuid,
    role_key: serde_json::Value,
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
struct HeldRole {
    assignment_id: Uuid,
    role_key: serde_json::Value,
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

/// Adds the principal `name` as a service account, refused when any principal has the name in any case.
pub(crate) async fn add_service_principal(
    transaction: ControlTransaction,
    context: &AuditContext,
    (name, description): (&str, Option<&str>),
) -> Result<(ControlTransaction, Uuid), ControlError> {
    let (transaction, taken) = query(
        transaction,
        "SELECT principal_id FROM sqb_principals WHERE lower(name)=$1",
        vec![Parameter::Text(name.into())],
    )
    .await?;
    if !taken.is_empty() {
        return Err(ControlError::Invalid(format!(
            "{name} already names a person or service account"
        )));
    }
    let (transaction, principal_id) =
        insert_principal(transaction, context, (Uuid::now_v7(), name)).await?;
    let description = description
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_service_accounts (principal_id, description, created_by) VALUES ($1,$2,$3)",
        vec![
            Parameter::Id(principal_id),
            Parameter::OptionalText(description),
            Parameter::Text(context.actor.clone()),
        ],
    )
    .await?;
    Ok((transaction, principal_id))
}

/// The service account `principal_id`, refused as invalid when it is a person or doesn't exist.
pub(crate) async fn require_service_account(
    transaction: ControlTransaction,
    principal_id: Uuid,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT principal_id FROM sqb_service_accounts WHERE principal_id=$1",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    if rows.is_empty() {
        return Err(ControlError::Invalid(format!(
            "no service account {principal_id}"
        )));
    }
    Ok(transaction)
}

/// A write changing `principal_id` that needs `user.manage`, refused when it isn't a service account.
pub(crate) async fn begin_account_change(
    store: &ControlStore,
    (context, authority): (&AuditContext, ServiceAccountAuthority),
    principal_id: Uuid,
) -> Result<(ControlTransaction, AuditContext), ControlError> {
    let manage = PermissionScope::action(Action::UserManage, None);
    let (transaction, authorized) = begin_change(store, (context, authority), &[manage]).await?;
    Ok((
        require_service_account(transaction, principal_id).await?,
        authorized,
    ))
}

/// Commits a change to `principal_id` and returns the account as it now reads.
pub(crate) async fn finish_change(
    transaction: ControlTransaction,
    principal_id: Uuid,
) -> Result<ServiceAccountSummary, ControlError> {
    let (transaction, account) = service_account_summary(transaction, principal_id).await?;
    commit(transaction).await?;
    Ok(account)
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
    commit(transaction).await?;
    let Some(row) = rows.into_iter().next() else {
        return Err(ControlError::Invalid(format!(
            "no service account named {name}"
        )));
    };
    Ok(decode_row::<PrincipalRow>(row)?.principal_id)
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

/// The built-in role a stored `role_key` names; custom roles name none.
pub(crate) fn builtin_role(role_key: &serde_json::Value) -> Option<BuiltinRole> {
    ROLE_ORDER.into_iter().find(|role| json!(role) == *role_key)
}

/// Each account's highest built-in role everywhere.
fn highest_roles(rows: Vec<Row>) -> Result<BTreeMap<Uuid, BuiltinRole>, ControlError> {
    let mut held: BTreeMap<Uuid, BuiltinRole> = BTreeMap::new();
    for row in rows {
        let row: RoleRow = decode_row(row)?;
        let Some(role) = builtin_role(&row.role_key) else {
            continue;
        };
        let rank = |role: &BuiltinRole| ROLE_ORDER.iter().position(|r| r == role);
        if held
            .get(&row.principal_id)
            .is_none_or(|current| rank(&role) > rank(current))
        {
            held.insert(row.principal_id, role);
        }
    }
    Ok(held)
}

/// Who made something, by name; none when the control-store owner did.
fn made_by(names: &Names, actor: &str) -> Option<String> {
    names.get(actor).map(|(name, _)| name.clone())
}

/// Whether a key works at `now`, or will once its account is enabled.
fn working(key: &KeyRow, now: DateTime<Utc>) -> bool {
    key.revoked_at.is_none() && key.expires_at.is_none_or(|end| end > now)
}

fn key_summary(key: &KeyRow, names: &Names) -> ServiceKeySummary {
    ServiceKeySummary {
        key_id: key.key_id,
        prefix: key.prefix.clone(),
        origin: key.origin,
        created_at: key.created_at,
        created_by: made_by(names, &key.created_by),
        last_used_at: key.last_used_at,
        expires_at: key.expires_at,
    }
}

/// One account with its working keys, the current one first, and its newest use of any key.
fn account_summary(
    account: AccountRow,
    (keys, roles, names): (&[KeyRow], &BTreeMap<Uuid, BuiltinRole>, &Names),
    now: DateTime<Utc>,
) -> ServiceAccountSummary {
    let mine: Vec<&KeyRow> = keys
        .iter()
        .filter(|key| key.principal_id == account.principal_id)
        .collect();
    let mut working_keys: Vec<ServiceKeySummary> = mine
        .iter()
        .filter(|key| working(key, now))
        .map(|key| key_summary(key, names))
        .collect();
    working_keys.sort_by_key(|key| key.expires_at.is_some());
    ServiceAccountSummary {
        principal_id: account.principal_id,
        role: roles.get(&account.principal_id).copied(),
        created_by: made_by(names, &account.created_by),
        name: account.name,
        description: account.description,
        created_at: account.created_at,
        disabled_at: account.disabled_at,
        last_used_at: mine.iter().filter_map(|key| key.last_used_at).max(),
        keys: working_keys,
    }
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
    let roles = highest_roles(roles)?;
    let keys: Vec<KeyRow> = keys
        .into_iter()
        .map(decode_row::<KeyRow>)
        .collect::<Result<_, _>>()?;
    let accounts: Vec<AccountRow> = accounts
        .into_iter()
        .map(decode_row::<AccountRow>)
        .collect::<Result<_, _>>()?;
    let summaries: Vec<ServiceAccountSummary> = accounts
        .into_iter()
        .map(|account| account_summary(account, (&keys, &roles, &names), now))
        .collect();
    Ok((transaction, summaries))
}

/// The one summary of `principal_id`, read in `transaction` after a change.
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

/// The summary of `principal_id`, read once a change has committed.
pub(crate) async fn read_service_account(
    store: &ControlStore,
    principal_id: Uuid,
) -> Result<ServiceAccountSummary, ControlError> {
    let (transaction, account) =
        service_account_summary(begin(store, TransactionMode::Read).await?, principal_id).await?;
    commit(transaction).await?;
    Ok(account)
}

/// Removes and audits every built-in role everywhere of `principal_id` other than `role`.
pub(crate) async fn revoke_other_roles(
    transaction: ControlTransaction,
    context: &AuditContext,
    (principal_id, role): (Uuid, BuiltinRole),
) -> Result<ControlTransaction, ControlError> {
    let (mut transaction, rows) = query(
        transaction,
        "SELECT assignment_id, role_key FROM sqb_role_assignments \
         WHERE principal_id=$1 AND location_id IS NULL AND target IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    for row in rows {
        let held_row: HeldRole = decode_row(row)?;
        let Some(held) = builtin_role(&held_row.role_key).filter(|held| *held != role) else {
            continue;
        };
        let removed;
        (transaction, removed) = query(
            transaction,
            "DELETE FROM sqb_role_assignments WHERE assignment_id=$1 RETURNING assignment_id",
            vec![Parameter::Id(held_row.assignment_id)],
        )
        .await?;
        transaction = append_audit(
            transaction,
            context,
            (Uuid::now_v7(), "role.revoked"),
            (
                json!({"principal_id": principal_id, "role": held, "location_id": null, "target": null}),
                json!(null),
                json!({"assignments": removed}),
            ),
        )
        .await?;
        transaction.changes.insert(Change::Credentials);
    }
    Ok(transaction)
}
