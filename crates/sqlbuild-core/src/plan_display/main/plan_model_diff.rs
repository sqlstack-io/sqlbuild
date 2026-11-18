//! One planned model's diff text, read from its plan document.

use serde_json::Value;

use crate::plan_display::_helpers::plan_pages::{lines, text};
use crate::plan_display::models::PlanModelDiff;

/// The named model's query and config diff text; `None` when the document has neither for it.
pub fn plan_model_diff(document: &Value, model: &str) -> Option<PlanModelDiff> {
    let row = document
        .get("models")?
        .as_array()?
        .iter()
        .find(|row| text(row, "name") == model)?;
    let diff = PlanModelDiff {
        query_diff: lines(row, "query_diff"),
        config_diff: lines(row, "config_diff"),
    };
    (diff.query_diff.is_some() || diff.config_diff.is_some()).then_some(diff)
}
