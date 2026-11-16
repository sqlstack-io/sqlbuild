//! The foreground `sqb debug` connection: its own lifecycle rows apart from the client's output, and its verdict.

use serde_json::Value;
use sqlbuild_core::screen_text::models::WarehouseState;

use crate::dev_session::constants::{
    ADAPTER_LABEL, CONNECTION_TEST_LABEL, ERROR_STATUS, LIFECYCLE_STATUSES, NO_REPORT, OK_STATUS,
    REPORT_ERROR_SECTIONS,
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
        let detail: Vec<String> = connection
            .iter()
            .filter(|row| row["status"].is_null())
            .map(|row| field(row, "message"))
            .filter(|message| !message.is_empty())
            .collect();
        return (
            adapter,
            WarehouseState::Connected {
                detail: detail.join(" \u{b7} "),
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
