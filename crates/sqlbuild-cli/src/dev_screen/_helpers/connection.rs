//! The foreground `sqb debug` connection: its own lifecycle rows apart from the client's output, and its verdict.

use serde_json::Value;
use sqlbuild_core::screen_text::models::WarehouseState;

use crate::dev_screen::constants::{CONNECTION_TEST_LABEL, LIFECYCLE_STATUSES, NO_REPORT};

/// SQLBuild's own progress row, such as `Project discovery  OK  (0.02s)`, never the client's.
pub(crate) fn is_lifecycle_row(line: &str) -> bool {
    let mut columns = line.split("  ");
    columns.next().is_some_and(|label| !label.trim().is_empty())
        && columns
            .next()
            .is_some_and(|status| LIFECYCLE_STATUSES.contains(&status))
}

/// Add `chunk` to `pending` and split off its complete lines: the client's text, then SQLBuild's rows.
pub(crate) fn split_debug_output(pending: &mut String, chunk: &str) -> (String, Vec<String>) {
    pending.push_str(chunk);
    let Some(end) = pending.rfind('\n') else {
        return (String::new(), Vec::new());
    };
    let complete: String = pending.drain(..=end).collect();
    let mut client: String = String::new();
    let mut rows: Vec<String> = Vec::new();
    for line in complete.split_inclusive('\n') {
        let row: &str = line.trim_end_matches(['\n', '\r']);
        if is_lifecycle_row(row) {
            rows.push(row.to_owned());
        } else {
            client.push_str(line);
        }
    }
    (client, rows)
}

/// The adapter and connection state a `sqb debug --json` report gives, or a failure without one.
pub(crate) fn connection_outcome(report: &[u8]) -> (String, WarehouseState) {
    let Ok(report) = serde_json::from_slice::<Value>(report) else {
        return (
            String::new(),
            WarehouseState::Failed {
                message: NO_REPORT.to_owned(),
            },
        );
    };
    let rows = |section: &str| -> Vec<Value> {
        report[section].as_array().cloned().unwrap_or_default()
    };
    let text = |row: &Value, field: &str| -> String {
        row[field].as_str().unwrap_or_default().to_owned()
    };
    let adapter: String = rows("configuration")
        .iter()
        .find(|row| row["label"] == "adapter")
        .map(|row| text(row, "message"))
        .unwrap_or_default();
    let connection: Vec<Value> = rows("connection");
    let test: Option<&Value> = connection
        .iter()
        .find(|row| row["label"] == CONNECTION_TEST_LABEL);
    if test.is_some_and(|row| row["status"] == "OK") {
        let detail: Vec<String> = connection
            .iter()
            .filter(|row| row.get("status").is_none())
            .map(|row| text(row, "message"))
            .filter(|message| !message.is_empty())
            .collect();
        return (
            adapter,
            WarehouseState::Connected {
                detail: detail.join(" \u{b7} "),
            },
        );
    }
    let message: String = test
        .into_iter()
        .chain(["configuration", "providers"].iter().flat_map(|section| {
            report[*section]
                .as_array()
                .map(|rows| rows.iter())
                .into_iter()
                .flatten()
        }))
        .find(|row| row["status"] == "ERROR")
        .map(|row| text(row, "status_message"))
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| NO_REPORT.to_owned());
    (adapter, WarehouseState::Failed { message })
}
