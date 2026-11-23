//! The one line each admin command prints for what it did; never a secret.

use chrono::{DateTime, SecondsFormat, Utc};
use sqlbuild_orchestration::credentials::models::{BuiltinRole, ServiceAccountSummary};
use sqlbuild_orchestration::declarations::models::TriggerChange;

use crate::native_commands::constants::NO_VALUE;
use crate::native_commands::models::AdminDone;

/// `None` for a read that found rows: there is nothing done to name.
pub(crate) fn summary_line(done: &AdminDone) -> Option<String> {
    let line: String = match done {
        AdminDone::Bootstrapped { id, name } => format!("Created admin user {name} ({id})"),
        AdminDone::LocationCreated { id, name } => format!("Registered location {name} ({id})"),
        AdminDone::PrincipalCreated { id, name } => {
            format!("Created person {name} ({id}) with no roles")
        }
        AdminDone::PasswordSet { id, name } => format!("Set the password of {name} ({id})"),
        AdminDone::AdminGranted { id, name } => format!("Made {name} ({id}) an admin"),
        AdminDone::TokenIssued { token, location } => format!(
            "Issued a location token for {location} ({}), expires {}",
            token.identity.token_id,
            until(token.identity.expires_at)
        ),
        AdminDone::TokenRotated { replaced, token } => format!(
            "Replaced token {replaced} with {}, expires {}",
            token.identity.token_id,
            until(token.identity.expires_at)
        ),
        AdminDone::TokenRevoked { id, revoked: true } => format!("Revoked token {id}"),
        AdminDone::TokenRevoked { id, revoked: false } => {
            format!("Token {id} was already revoked or doesn't exist; nothing changed")
        }
        AdminDone::RunQueued {
            run_id,
            job,
            target,
            applied: true,
        } => {
            format!("Queued a run of {job} on target {target} ({run_id})")
        }
        AdminDone::RunQueued {
            run_id,
            applied: false,
            ..
        }
        | AdminDone::RerunQueued {
            run_id,
            applied: false,
            ..
        } => {
            format!("Run {run_id} was already requested; nothing changed")
        }
        AdminDone::RerunQueued {
            run_id,
            source,
            applied: true,
        } => {
            format!("Queued a rerun of run {source} ({run_id})")
        }
        AdminDone::CheckRequested {
            request_id,
            location,
            target,
            applied: true,
        } => {
            format!("Requested a connection check of {location} on target {target} ({request_id})")
        }
        AdminDone::CheckRequested {
            request_id,
            applied: false,
            ..
        } => {
            format!("Connection check {request_id} was already requested; nothing changed")
        }
        AdminDone::WorkRead {
            request_id,
            record: None,
            ..
        } => {
            format!("No location request {request_id}")
        }
        AdminDone::WorkRead {
            record: Some(_), ..
        } => return None,
        AdminDone::CancelRequested {
            event_id,
            run_id,
            applied: true,
        } => {
            format!("Requested cancellation of run {run_id} ({event_id})")
        }
        AdminDone::CancelRequested {
            event_id,
            run_id,
            applied: false,
        } => format!(
            "Cancellation of run {run_id} ({event_id}) was already requested; nothing changed"
        ),
        AdminDone::TriggerChanged {
            event_id,
            trigger_id,
            applied: false,
            ..
        } => format!(
            "The change to trigger {trigger_id} ({event_id}) was already made; nothing changed"
        ),
        AdminDone::TriggerChanged {
            trigger_id,
            change,
            applied: true,
            ..
        } => match change {
            TriggerChange::Enable => format!("Enabled trigger {trigger_id}"),
            TriggerChange::Pause => format!("Paused trigger {trigger_id}"),
            TriggerChange::ResetDefault => {
                format!("Reset trigger {trigger_id} to its default status")
            }
            TriggerChange::SetCursor { time } => format!(
                "Set the cursor of trigger {trigger_id} to {}",
                time.to_rfc3339_opts(SecondsFormat::AutoSi, true)
            ),
            TriggerChange::ResetCursor => format!("Reset the cursor of trigger {trigger_id}"),
            TriggerChange::SkipEvent { event } => {
                format!("Skipped event {event} of trigger {trigger_id}")
            }
        },
        AdminDone::AccountCreated { account, key } => format!(
            "Created service account {} ({}) with role {}{}",
            account.name,
            account.principal_id,
            role_text(account.role),
            match key {
                Some(_) => "; its key is shown only this once",
                None => " and the imported key",
            }
        ),
        AdminDone::KeyRotated { account, .. } => rotated(account),
        AdminDone::AccountSwitched { account, disabled } => format!(
            "{} service account {} ({})",
            if *disabled { "Disabled" } else { "Enabled" },
            account.name,
            account.principal_id
        ),
        AdminDone::AccountsListed(accounts) => {
            return accounts
                .is_empty()
                .then(|| "No service accounts".to_owned());
        }
    };
    Some(printable(&line))
}

fn rotated(account: &ServiceAccountSummary) -> String {
    let overlap: String = account
        .keys
        .get(1)
        .and_then(|key| key.expires_at)
        .map_or_else(
            || "has stopped".to_owned(),
            |stops| format!("works until {}", until(stops)),
        );
    format!(
        "Rotated the key of service account {} ({}); the replaced key {overlap}",
        account.name, account.principal_id
    )
}

fn until(time: DateTime<Utc>) -> String {
    time.format("%Y-%m-%d %H:%M UTC").to_string()
}

pub(crate) fn role_text(role: Option<BuiltinRole>) -> String {
    serde_json::json!(role)
        .as_str()
        .unwrap_or(NO_VALUE)
        .to_owned()
}

/// A stored label can't add lines or terminal controls.
pub(crate) fn printable(text: &str) -> String {
    text.chars()
        .map(|letter| {
            if letter.is_control() {
                letter.escape_default().to_string()
            } else {
                letter.to_string()
            }
        })
        .collect()
}
