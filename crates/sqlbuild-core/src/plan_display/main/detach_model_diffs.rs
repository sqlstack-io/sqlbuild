//! Take model diff text out of a plan document, leaving each model the counts of its diffs.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::plan_display::_helpers::diff_lines::config_change;
use crate::plan_display::_helpers::plan_pages::{lines, row_diff_stats, text};
use crate::plan_display::constants::{CONFIG_CHANGE_FIELD, DIFF_STATS_FIELD, MODEL_DIFF_FIELDS};

/// The document without model diff text, each model keeping its `diff_stats` and short `config_change`, and by model the `query_diff` and `config_diff` fields taken out.
pub fn detach_model_diffs(mut document: Value) -> (Value, BTreeMap<String, Value>) {
    let mut diffs: BTreeMap<String, Value> = BTreeMap::new();
    let models = document.get_mut("models").and_then(Value::as_array_mut);
    for model in models.into_iter().flatten() {
        let mut diff: Map<String, Value> = Map::new();
        for field in MODEL_DIFF_FIELDS {
            if let Some(text) = lines(model, field) {
                diff.insert(field.to_owned(), Value::from(text));
            }
        }
        if diff.is_empty() {
            continue;
        }
        let stats = row_diff_stats(model);
        let change = lines(model, "config_diff")
            .as_deref()
            .and_then(config_change);
        diffs.insert(text(model, "name").to_owned(), Value::Object(diff));
        if let Some(fields) = model.as_object_mut() {
            for field in MODEL_DIFF_FIELDS {
                fields.remove(field);
            }
            fields.insert(DIFF_STATS_FIELD.to_owned(), Value::Object(stats));
            if let Some(change) = change {
                fields.insert(CONFIG_CHANGE_FIELD.to_owned(), Value::String(change));
            }
        }
    }
    (document, diffs)
}
