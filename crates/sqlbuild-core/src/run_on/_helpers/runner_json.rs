//! Decisions and refusals read from the local runner's JSON objects.

use serde_json::Value;

use crate::run_on::models::{LocalRefusal, RunOnDecision};

/// One decision object; missing fields are empty.
pub(crate) fn decision(value: &Value) -> RunOnDecision {
    let refusals: &[Value] = value
        .get("refusals")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);
    RunOnDecision {
        name: text(value, "name").unwrap_or_default(),
        run_on: text(value, "run_on").unwrap_or_default(),
        refusals: refusals.iter().map(refusal).collect(),
        relied_on: strings(value, "relied_on"),
    }
}

fn refusal(value: &Value) -> LocalRefusal {
    LocalRefusal {
        category: text(value, "category").unwrap_or_default(),
        construct: text(value, "construct").unwrap_or_default(),
        reason: text(value, "reason").unwrap_or_default(),
        code: text(value, "code"),
        message: text(value, "message"),
        label: text(value, "label"),
        notes: strings(value, "notes"),
        help: text(value, "help"),
        path: text(value, "path"),
        line: number(value, "line"),
        column: number(value, "column"),
        width: number(value, "width").unwrap_or(1),
        snippet: text(value, "snippet"),
        snippet_column: number(value, "snippet_column"),
        expanded_from: text(value, "expanded_from"),
    }
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn number(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    let items: &[Value] = value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);
    items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
