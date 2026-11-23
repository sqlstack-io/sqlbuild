//! Each admin command's JSON result, which `--json` prints.

use serde_json::{Value, json};
use sqlbuild_orchestration::credentials::models::ServiceAccountSummary;

use crate::native_commands::models::AdminDone;

pub(crate) fn result_json(done: &AdminDone) -> Value {
    match done {
        AdminDone::Bootstrapped { id, name } => {
            json!({"principal_id": id, "name": name, "kind": "person", "role": "admin"})
        }
        AdminDone::LocationCreated { id, name } => json!({"location_id": id, "name": name}),
        AdminDone::PrincipalCreated { id, name } => json!({"principal_id": id, "name": name}),
        AdminDone::PasswordSet { id, .. } => json!({"principal_id": id, "password_set": true}),
        AdminDone::AdminGranted { id, .. } => json!({"principal_id": id, "role": "admin"}),
        AdminDone::TokenIssued { token, .. } | AdminDone::TokenRotated { token, .. } => {
            json!(token)
        }
        AdminDone::TokenRevoked { id, revoked } => json!({"token_id": id, "revoked": revoked}),
        AdminDone::RunQueued {
            run_id, applied, ..
        }
        | AdminDone::RerunQueued {
            run_id, applied, ..
        } => {
            json!({"run_id": run_id, "applied": applied})
        }
        AdminDone::CheckRequested {
            request_id,
            applied,
            ..
        } => {
            json!({"request_id": request_id, "applied": applied})
        }
        AdminDone::WorkRead { record, .. } => json!(record),
        AdminDone::CancelRequested {
            event_id,
            run_id,
            applied,
        } => {
            json!({"event_id": event_id, "run_id": run_id, "applied": applied})
        }
        AdminDone::TriggerChanged {
            event_id,
            trigger_id,
            applied,
            ..
        } => {
            json!({"event_id": event_id, "trigger_id": trigger_id, "applied": applied})
        }
        AdminDone::AccountCreated { account, key } => key_result(account, key.as_ref()),
        AdminDone::KeyRotated { account, key } => key_result(account, Some(key)),
        AdminDone::AccountSwitched { account, .. } => json!(account),
        AdminDone::AccountsListed(accounts) => json!(accounts),
    }
}

/// The account with its new key, or without one when the operator imported their own key.
fn key_result(account: &ServiceAccountSummary, key: Option<&String>) -> Value {
    let mut result = json!({
        "principal_id": account.principal_id,
        "name": account.name,
        "role": account.role,
        "key_prefix": account.keys.first().map(|key| &key.prefix),
        "account": account,
    });
    if let Some(key) = key {
        result["key"] = json!(key);
    }
    result
}
