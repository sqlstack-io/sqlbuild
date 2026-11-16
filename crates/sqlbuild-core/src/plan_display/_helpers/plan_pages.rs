//! Which models a plan document builds, and which of them are routine, as the plan page reads them.

use std::collections::HashSet;

use serde_json::Value;

use crate::plan_display::constants::{ROUTINE_PLAN_REASONS, SKIP_ACTION};

/// The node keys a `--changed` plan runs; `None` without `--changed`, when every planned node runs.
pub(crate) fn changed_runs(document: &Value) -> Option<HashSet<&str>> {
    let runs = document.get("changed")?.get("runs")?.as_array()?;
    Some(runs.iter().filter_map(run_node).collect())
}

/// The node key a `--changed` run names.
pub(crate) fn run_node(run: &Value) -> Option<&str> {
    run.get("node").and_then(Value::as_str)
}

fn text<'value>(entry: &'value Value, field: &str) -> &'value str {
    entry.get(field).and_then(Value::as_str).unwrap_or_default()
}

/// The model's node key, such as `model:orders`.
pub(crate) fn model_key(model: &Value) -> String {
    format!("model:{}", text(model, "name"))
}

/// Whether the run builds `model`: the planner does not skip it, and `--changed` runs it.
pub(crate) fn built(model: &Value, runs: Option<&HashSet<&str>>) -> bool {
    text(model, "action") != SKIP_ACTION
        && runs.is_none_or(|runs| runs.contains(model_key(model).as_str()))
}

/// Whether the model builds as it always does, with no change of its own.
pub(crate) fn routine(model: &Value) -> bool {
    ROUTINE_PLAN_REASONS.contains(&text(model, "reason"))
}

/// The document's models, or none for a failure report.
pub(crate) fn models(document: &Value) -> &[Value] {
    document
        .get("models")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// The built routine models in plan order.
pub(crate) fn routine_models<'document>(
    document: &'document Value,
    runs: Option<&HashSet<&str>>,
) -> Vec<&'document Value> {
    models(document)
        .iter()
        .filter(|model| built(model, runs) && routine(model))
        .collect()
}
