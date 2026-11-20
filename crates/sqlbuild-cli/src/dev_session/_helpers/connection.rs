//! The foreground `sqb debug` connection: its own lifecycle rows apart from the client's output, and its verdict.

use serde_json::Value;
use sqlbuild_core::screen_text::models::WarehouseState;

use crate::dev_session::constants::{
    ADAPTER_LABEL, AUTH_KEYS, AUTH_NAMES, CONNECTION_TEST_LABEL, ERROR_STATUS, KEY_PAIR_KEYS,
    LIFECYCLE_STATUSES, NO_REPORT, OK_STATUS, PLACE_KEYS, REPORT_ERROR_SECTIONS, ROLE_KEYS,
    STORAGE_KEYS, UNSHOWN_VALUES,
};

/// SQLBuild's own progress row, such as `Project discovery  OK  (0.02s)`, never the client's.
pub(crate) fn is_lifecycle_row(line: &str) -> bool {
    let mut columns = line.split("  ");
    columns.next().is_some_and(|label| !label.trim().is_empty())
        && columns
            .next()
            .is_some_and(|status| LIFECYCLE_STATUSES.contains(&status))
}

/// `chunk` split into the client's text, kept as written, and SQLBuild's own complete rows.
pub(crate) fn split_debug_output(chunk: &str) -> (String, Vec<String>) {
    let mut client: String = String::new();
    let mut rows: Vec<String> = Vec::new();
    for line in chunk.split_inclusive('\n') {
        let row: &str = line.trim_end_matches(['\n', '\r']);
        if line.ends_with('\n') && is_lifecycle_row(row) {
            rows.push(row.to_owned());
        } else {
            client.push_str(line);
        }
    }
    (client, rows)
}

/// The rows of one report section.
fn section(report: &Value, name: &str) -> Vec<Value> {
    report[name].as_array().cloned().unwrap_or_default()
}

/// One text field of a report row, empty when absent.
fn field(row: &Value, name: &str) -> String {
    row[name].as_str().unwrap_or_default().to_owned()
}

/// The adapter and connection state a `sqb debug --json` report gives, or a failure without one.
pub(crate) fn connection_outcome(report: &[u8]) -> (String, WarehouseState) {
    let report: Value = match serde_json::from_slice(report) {
        Ok(report) => report,
        Err(_) => {
            return (
                String::new(),
                WarehouseState::Failed {
                    message: NO_REPORT.to_owned(),
                },
            );
        }
    };
    let adapter: String = section(&report, "configuration")
        .iter()
        .find(|row| row["label"] == ADAPTER_LABEL)
        .map(|row| field(row, "message"))
        .unwrap_or_default();
    let connection: Vec<Value> = section(&report, "connection");
    let test: Option<Value> = connection
        .iter()
        .find(|row| row["label"] == CONNECTION_TEST_LABEL)
        .cloned();
    if test.as_ref().is_some_and(|row| row["status"] == OK_STATUS) {
        let settings: Vec<(String, String)> = connection
            .iter()
            .filter(|row| row["status"].is_null())
            .map(|row| (field(row, "label").to_lowercase(), field(row, "message")))
            .collect();
        return (
            adapter,
            WarehouseState::Connected {
                facts: connection_facts(&settings),
            },
        );
    }
    let mut candidates: Vec<Value> = test.into_iter().collect();
    for name in REPORT_ERROR_SECTIONS {
        candidates.extend(section(&report, name));
    }
    let message: String = candidates
        .iter()
        .find(|row| row["status"] == ERROR_STATUS)
        .map(|row| field(row, "status_message"))
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| NO_REPORT.to_owned());
    (adapter, WarehouseState::Failed { message })
}

/// The connection's labelled settings in a fixed order: where (account or host), auth, role, then the warehouse or database. Unknown, masked, boolean and nested values are left out.
pub(crate) fn connection_facts(settings: &[(String, String)]) -> Vec<String> {
    let mut facts: Vec<String> = Vec::new();
    if let Some((label, place)) = setting(settings, &PLACE_KEYS) {
        facts.push(format!("{label} {place}"));
    }
    let auth: Option<String> = setting(settings, &AUTH_KEYS)
        .map(|(_, method)| auth_name(&method))
        .or_else(|| setting(settings, &KEY_PAIR_KEYS).map(|_| "key pair".to_owned()));
    if let Some(auth) = auth {
        facts.push(format!("auth {auth}"));
    }
    if let Some((_, role)) = setting(settings, &ROLE_KEYS) {
        facts.push(format!("role {role}"));
    }
    if let Some((label, storage)) = setting(settings, &STORAGE_KEYS) {
        facts.push(format!("{label} {storage}"));
    }
    facts
}

/// The first of `keys` that `settings` has with a value worth showing, and that value.
fn setting(settings: &[(String, String)], keys: &[&str]) -> Option<(String, String)> {
    for key in keys {
        for (label, value) in settings {
            if label == key && shown(value) {
                return Some((label.clone(), value.clone()));
            }
        }
    }
    None
}

/// A readable name for an authenticator, such as `browser sign-in (OAuth)` for `OAUTH_AUTHORIZATION_CODE`.
fn auth_name(method: &str) -> String {
    let method: String = method.to_lowercase();
    AUTH_NAMES
        .iter()
        .find(|(name, _)| *name == method)
        .map_or(method.clone(), |(_, readable)| (*readable).to_owned())
}

/// Whether a setting's value says something: not empty, masked, a boolean, `None` or a nested setting.
fn shown(value: &str) -> bool {
    let value: &str = value.trim();
    !value.is_empty()
        && !UNSHOWN_VALUES.contains(&value.to_lowercase().as_str())
        && !value.starts_with('{')
        && !value.starts_with('[')
        && !value.chars().all(|character| character == '*')
}

/// `sqb debug --json`'s stdout split into the client's text, such as a sign-in prompt, and the report that follows it.
#[derive(Debug, Default)]
pub(crate) struct ReportSplit {
    report: String,
    in_report: bool,
    partial: String,
}

impl ReportSplit {
    /// The client's text in `chunk`; the report begins at the first line that starts with `{`.
    pub(crate) fn feed(&mut self, chunk: &str) -> String {
        if self.in_report {
            self.report.push_str(chunk);
            return String::new();
        }
        let text: String = format!("{}{chunk}", std::mem::take(&mut self.partial));
        let mut client: String = String::new();
        for piece in text.split_inclusive('\n') {
            if self.in_report {
                self.report.push_str(piece);
            } else if piece.starts_with('{') {
                if piece.ends_with('\n') {
                    self.in_report = true;
                    self.report.push_str(piece);
                } else {
                    self.partial.push_str(piece);
                }
            } else {
                client.push_str(piece);
            }
        }
        client
    }

    /// The report's bytes.
    pub(crate) fn finish(self) -> Vec<u8> {
        format!("{}{}", self.report, self.partial).into_bytes()
    }
}
