//! A plan document's summary: everything but its large sections, which it counts for paging.

use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::plan_display::_helpers::plan_pages::{
    built, changed_runs, model_key, models, routine, routine_models, run_node,
};
use crate::plan_display::constants::PAGED_CHANGED_SECTIONS;

/// The summary of `document`: built changed models in full with every built model's name, routine
/// models, held-back and unchanged nodes as `pages` counts, and `--changed` runs of the models it
/// carries; per-model metadata is left out. A failure report is returned unchanged.
pub fn plan_summary(document: &Value) -> Value {
    let Some(fields) = document.as_object() else {
        return document.clone();
    };
    if !fields.contains_key("models") {
        return document.clone();
    }
    let runs = changed_runs(document);
    let routine_rows = routine_models(document, runs.as_ref());
    let routine_keys: HashSet<String> = routine_rows.iter().map(|model| model_key(model)).collect();
    let mut summary: Map<String, Value> = fields
        .iter()
        .filter(|(field, _)| !matches!(field.as_str(), "models" | "metadata" | "changed"))
        .map(|(field, value)| (field.clone(), value.clone()))
        .collect();
    let built_models: Vec<&Value> = models(document)
        .iter()
        .filter(|model| built(model, runs.as_ref()))
        .collect();
    summary.insert(
        "models".into(),
        Value::Array(
            built_models
                .iter()
                .filter(|model| !routine(model))
                .map(|model| (*model).clone())
                .collect(),
        ),
    );
    summary.insert(
        "built_model_names".into(),
        Value::Array(
            built_models
                .iter()
                .filter_map(|model| model.get("name").cloned())
                .collect(),
        ),
    );
    let mut pages: Map<String, Value> = Map::new();
    pages.insert("routine_models".into(), json!(routine_rows.len()));
    if let Some(changed) = document.get("changed").and_then(Value::as_object) {
        let mut kept: Map<String, Value> = changed.clone();
        if let Some(Value::Array(all_runs)) = kept.get_mut("runs") {
            all_runs.retain(|run| run_node(run).is_none_or(|node| !routine_keys.contains(node)));
        }
        for section in PAGED_CHANGED_SECTIONS {
            let count: usize = changed
                .get(section)
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            pages.insert(section.into(), json!(count));
            kept.insert(section.into(), json!([]));
        }
        summary.insert("changed".into(), Value::Object(kept));
    } else if fields.contains_key("changed") {
        summary.insert("changed".into(), Value::Null);
    }
    summary.insert("pages".into(), Value::Object(pages));
    Value::Object(summary)
}
