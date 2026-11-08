//! Administrators who manage people list every principal with sign-in facts, never secrets.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, PermissionScope, PersonSummary,
};
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, Parameter};
use crate::store::types::Row;

/// Everyone by name, with sessions still live at `now`; requires `user.manage` everywhere.
pub async fn list_people(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    now: DateTime<Utc>,
) -> Result<Vec<PersonSummary>, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        (context, credentials),
        &PermissionScope::action(Action::UserManage, None),
        DefaultRole::None,
    )
    .await?;
    let (transaction, people) = query(transaction,
        "SELECT p.principal_id, p.name, p.created_at, p.disabled_at, \
           CASE WHEN c.principal_id IS NULL THEN 0 ELSE 1 END AS has_password, \
           (SELECT COUNT(*) FROM sqb_sessions s WHERE s.principal_id=p.principal_id \
              AND s.revoked_at IS NULL AND s.expires_at>$1 AND p.disabled_at IS NULL) AS active_sessions, \
           (SELECT MAX(s.created_at) FROM sqb_sessions s WHERE s.principal_id=p.principal_id) AS last_signed_in_at \
         FROM sqb_principals p LEFT JOIN sqb_password_credentials c ON c.principal_id=p.principal_id ORDER BY p.name",
        vec![Parameter::Time(now)]).await?;
    let (transaction, memberships) = query(
        transaction,
        "SELECT principal_id, group_name FROM sqb_person_groups ORDER BY group_name",
        Vec::new(),
    )
    .await?;
    commit(transaction).await?;
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in &memberships {
        if let (Some(id), Some(group)) = (row["principal_id"].as_str(), row["group_name"].as_str())
        {
            groups.entry(id.into()).or_default().push(group.into());
        }
    }
    people
        .into_iter()
        .map(|row| person_of(row, &groups))
        .collect()
}

fn person_of(
    mut row: Row,
    groups: &BTreeMap<String, Vec<String>>,
) -> Result<PersonSummary, ControlError> {
    let member_of = row["principal_id"]
        .as_str()
        .and_then(|id| groups.get(id))
        .cloned()
        .unwrap_or_default();
    let has_password = row["has_password"].as_i64() == Some(1);
    row.insert("groups".into(), json!(member_of));
    row.insert("has_password".into(), json!(has_password));
    decode_row(row)
}
