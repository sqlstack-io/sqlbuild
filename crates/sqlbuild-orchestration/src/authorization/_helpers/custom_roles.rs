//! Published custom roles grant only non-administrative actions.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::credentials::models::{Action, RoleDefinition};
use crate::store::errors::ControlError;

const ADMINISTRATIVE: [Action; 3] = [Action::UserManage, Action::RoleAssign, Action::AuditView];

const BUILTIN: [&str; 4] = ["viewer", "operator", "developer", "admin"];

fn action_name(action: Action) -> Result<String, ControlError> {
    match serde_json::to_value(action) {
        Ok(Value::String(name)) => Ok(name),
        _ => Err(ControlError::Invalid("unnamed permission action".into())),
    }
}

pub(crate) fn decode_roles(
    document: Option<&Value>,
) -> Result<BTreeMap<String, BTreeSet<String>>, ControlError> {
    let roles: BTreeMap<String, RoleDefinition> = document.map_or_else(
        || Ok(BTreeMap::new()),
        |document| {
            serde_json::from_value(document.clone())
                .map_err(|error| ControlError::Invalid(format!("roles: {error}")))
        },
    )?;
    let mut published: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (key, definition) in roles {
        if key.trim().is_empty()
            || BUILTIN.contains(&key.to_lowercase().as_str())
            || definition.description.trim().is_empty()
            || definition
                .actions
                .iter()
                .any(|action| ADMINISTRATIVE.contains(action))
        {
            return Err(ControlError::Invalid(format!(
                "role {key} needs a description, a key that is not a built-in role, and only non-administrative actions"
            )));
        }
        let mut grants: BTreeSet<String> = BTreeSet::new();
        for action in definition.actions {
            grants.insert(action_name(action)?);
        }
        published.insert(key, grants);
    }
    Ok(published)
}

pub(crate) fn decode_grants(value: &Value) -> Result<BTreeSet<String>, ControlError> {
    serde_json::from_str(value.as_str().unwrap_or("[]"))
        .map_err(|error| ControlError::Schema(format!("effective role grants: {error}")))
}

pub(crate) fn grants_allow(value: &Value, action: Action) -> bool {
    match (decode_grants(value), action_name(action)) {
        (Ok(grants), Ok(name)) => grants.contains(&name),
        _ => false,
    }
}
