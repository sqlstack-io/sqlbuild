//! The decisions the local runner's JSON reports, one per test or scenario.

use serde_json::Value;

use crate::run_on::models::{LocalRefusal, RunOnDecision};

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn number(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn refusal(value: &Value) -> LocalRefusal {
    LocalRefusal {
        category: text(value, "category").unwrap_or_default(),
        construct: text(value, "construct").unwrap_or_default(),
        reason: text(value, "reason").unwrap_or_default(),
        code: text(value, "code"),
        message: text(value, "message"),
        label: text(value, "label"),
        notes: value
            .get("notes")
            .and_then(Value::as_array)
            .map(|notes| notes.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default(),
        help: text(value, "help"),
        path: text(value, "path"),
        line: number(value, "line"),
        column: number(value, "column"),
        width: number(value, "width").unwrap_or(1),
        snippet: text(value, "snippet"),
        expanded_from: text(value, "expanded_from"),
    }
}

/// `{"decisions": [{name, run_on, refusals: [...]}, ...]}` as decisions; malformed JSON is an error.
pub fn read_run_on_decisions(json: &str) -> Result<Vec<RunOnDecision>, String> {
    let document: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let decisions: &Vec<Value> = document
        .get("decisions")
        .and_then(Value::as_array)
        .ok_or_else(|| "run-on decisions have no `decisions` list".to_owned())?;
    Ok(decisions
        .iter()
        .map(|decision| RunOnDecision {
            name: text(decision, "name").unwrap_or_default(),
            run_on: text(decision, "run_on").unwrap_or_default(),
            refusals: decision
                .get("refusals")
                .and_then(Value::as_array)
                .map(|refusals| refusals.iter().map(refusal).collect())
                .unwrap_or_default(),
        })
        .collect())
}
