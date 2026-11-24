//! A `--json` document with where each item ran, why, and every refusal with the items it blocks.

use serde_json::{Map, Value, json};

use crate::run_on::models::{LocalRefusal, RunOnDecision};

/// `payload` with `run_on` and `run_on_reason` on each item, the split in `summary`, and `refusals`.
pub fn with_run_on_json(
    payload: &str,
    (items_key, name_keys): (&str, &[&str]),
    decisions: &[RunOnDecision],
    (local_count, warehouse_count): (usize, usize),
) -> Result<String, String> {
    let mut document: Value = serde_json::from_str(payload).map_err(|error| error.to_string())?;
    if let Some(items) = document.get_mut(items_key).and_then(Value::as_array_mut) {
        for item in items {
            let name: Option<String> = name_keys
                .iter()
                .find_map(|key| item.get(*key).and_then(Value::as_str))
                .map(str::to_owned);
            let Some(decision) = name
                .as_deref()
                .and_then(|name| decisions.iter().find(|decision| decision.name == name))
            else {
                continue;
            };
            if let Some(fields) = item.as_object_mut() {
                fields.insert("run_on".to_owned(), json!(decision.run_on));
                fields.insert("run_on_reason".to_owned(), json!(decision.reason()));
            }
        }
    }
    if let Some(summary) = document.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("local_count".to_owned(), json!(local_count));
        summary.insert("warehouse_count".to_owned(), json!(warehouse_count));
    }
    if let Some(fields) = document.as_object_mut() {
        fields.insert("refusals".to_owned(), Value::Array(refusals(decisions)));
    }
    serde_json::to_string(&document).map_err(|error| error.to_string())
}

/// Each refusal once per construct and place, as SQLQuack's `--json` reports it, with its items.
fn refusals(decisions: &[RunOnDecision]) -> Vec<Value> {
    let mut found: Vec<(LocalRefusal, Vec<String>)> = Vec::new();
    for decision in decisions {
        for refusal in &decision.refusals {
            let same = |known: &LocalRefusal| {
                known.code == refusal.code
                    && known.construct == refusal.construct
                    && known.path == refusal.path
                    && known.line == refusal.line
                    && known.column == refusal.column
            };
            match found.iter_mut().find(|(known, _)| same(known)) {
                Some((_, names)) if !names.contains(&decision.name) => {
                    names.push(decision.name.clone());
                }
                Some(_) => {}
                None => found.push((refusal.clone(), vec![decision.name.clone()])),
            }
        }
    }
    found
        .into_iter()
        .map(|(refusal, names)| {
            let mut fields: Map<String, Value> = Map::new();
            fields.insert("code".to_owned(), json!(refusal.code));
            fields.insert("category".to_owned(), json!(refusal.category));
            fields.insert("construct".to_owned(), json!(refusal.construct));
            fields.insert("title".to_owned(), json!(refusal.title()));
            fields.insert("reason".to_owned(), json!(refusal.reason));
            fields.insert("message".to_owned(), json!(refusal.message));
            fields.insert("label".to_owned(), json!(refusal.label));
            fields.insert("notes".to_owned(), json!(refusal.notes));
            fields.insert("help".to_owned(), json!(refusal.help));
            fields.insert("path".to_owned(), json!(refusal.path));
            fields.insert("line".to_owned(), json!(refusal.line));
            fields.insert("column".to_owned(), json!(refusal.column));
            fields.insert("tests".to_owned(), json!(names));
            Value::Object(fields)
        })
        .collect()
}
