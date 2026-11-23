//! An admin command's stdout without `--json`: a bare secret, or a read's plain rows.

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::json;
use sqlbuild_core::text::main::cell_len::cell_len;
use sqlbuild_orchestration::credentials::models::ServiceAccountSummary;
use sqlbuild_orchestration::platform::models::{LocationWorkOutcome, LocationWorkRecord};

use crate::native_commands::_helpers::admin::summaries::{printable, role_text};
use crate::native_commands::constants::NO_VALUE;
use crate::native_commands::models::AdminDone;

/// Empty for a command that only did something, so nothing but a secret or a read reaches stdout.
pub(crate) fn stdout_text(done: &AdminDone) -> String {
    match done {
        AdminDone::TokenIssued { token, .. } | AdminDone::TokenRotated { token, .. } => {
            format!("{}\n", token.bearer)
        }
        AdminDone::AccountCreated { key: Some(key), .. } | AdminDone::KeyRotated { key, .. } => {
            format!("{key}\n")
        }
        AdminDone::WorkRead {
            record: Some(record),
            now,
            ..
        } => rows_text(work_rows(record, *now)),
        AdminDone::AccountsListed(accounts) => {
            rows_text(accounts.iter().map(account_row).collect())
        }
        _ => String::new(),
    }
}

/// Name, role, state, the prefixes of working keys and the ID.
fn account_row(account: &ServiceAccountSummary) -> Vec<String> {
    let prefixes: Vec<&str> = account.keys.iter().map(|key| key.prefix.as_str()).collect();
    vec![
        account.name.clone(),
        role_text(account.role),
        if account.disabled_at.is_some() {
            "disabled"
        } else {
            "active"
        }
        .to_owned(),
        if prefixes.is_empty() {
            NO_VALUE.to_owned()
        } else {
            prefixes.join(",")
        },
        account.principal_id.to_string(),
    ]
}

/// One row per fact of a location request, its state derived at `now`.
fn work_rows(record: &LocationWorkRecord, now: DateTime<Utc>) -> Vec<Vec<String>> {
    let request = &record.request_json;
    let state: &str = if record.completed_at.is_some() {
        "completed"
    } else if record.expires_at <= now {
        "expired"
    } else {
        "waiting"
    };
    let outcome: String = record.result_json.as_ref().map_or_else(
        || NO_VALUE.to_owned(),
        |result| match &result.outcome {
            LocationWorkOutcome::Connection { healthy: true } => "connection healthy".to_owned(),
            LocationWorkOutcome::Connection { healthy: false } => "connection failed".to_owned(),
            outcome => kind_text(&json!(outcome)),
        },
    );
    let completed: String = record
        .completed_at
        .map_or_else(|| NO_VALUE.to_owned(), time_text);
    [
        ("request_id", request.request_id.to_string()),
        ("work", kind_text(&json!(request.work))),
        ("location_id", request.location_id.to_string()),
        ("target", request.target.clone()),
        ("state", state.to_owned()),
        ("outcome", outcome),
        ("requested_by", record.requested_by.clone()),
        ("requested_at", time_text(record.requested_at)),
        ("expires_at", time_text(record.expires_at)),
        ("completed_at", completed),
        ("reason", request.reason.clone()),
    ]
    .into_iter()
    .map(|(name, value)| vec![name.to_owned(), value])
    .collect()
}

fn kind_text(value: &serde_json::Value) -> String {
    value["kind"].as_str().unwrap_or(NO_VALUE).to_owned()
}

fn time_text(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Columns two spaces apart, with no header and one line per row.
fn rows_text(rows: Vec<Vec<String>>) -> String {
    let cells: Vec<Vec<String>> = rows.iter().map(|row| printable_row(row)).collect();
    let mut widths: Vec<usize> = Vec::new();
    for row in &cells {
        widths.resize(widths.len().max(row.len()), 0);
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell_len(cell));
        }
    }
    let mut text: String = String::new();
    for row in &cells {
        let mut line: String = String::new();
        for (width, cell) in widths.iter().zip(row) {
            line.push_str(cell);
            line.push_str(&" ".repeat(width - cell_len(cell) + 2));
        }
        text.push_str(line.trim_end());
        text.push('\n');
    }
    text
}

fn printable_row(row: &[String]) -> Vec<String> {
    row.iter().map(|cell| printable(cell)).collect()
}
