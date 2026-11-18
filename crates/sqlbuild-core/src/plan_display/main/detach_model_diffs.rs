//! Take model diff text out of a plan document, leaving each model the counts of its diffs.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::plan_display::_helpers::diff_lines::config_change;
use crate::plan_display::_helpers::plan_pages::{lines, row_diff_stats, text};
use crate::plan_display::constants::{CONFIG_CHANGE_FIELD, DIFF_STATS_FIELD, MODEL_DIFF_FIELDS};
use crate::plan_display::models::PlanModelDiff;

/// The document without model diff text, each model keeping its `diff_stats` and short `config_change`, and the text by model.
pub fn detach_model_diffs(mut document: Value) -> (Value, BTreeMap<String, PlanModelDiff>) {
    let mut diffs: BTreeMap<String, PlanModelDiff> = BTreeMap::new();
    let models = document.get_mut("models").and_then(Value::as_array_mut);
    for model in models.into_iter().flatten() {
        let diff = PlanModelDiff {
            query_diff: lines(model, "query_diff"),
            config_diff: lines(model, "config_diff"),
        };
        if diff.query_diff.is_none() && diff.config_diff.is_none() {
            continue;
        }
        let stats = row_diff_stats(model);
        let change = diff.config_diff.as_deref().and_then(config_change);
        diffs.insert(text(model, "name").to_owned(), diff);
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
