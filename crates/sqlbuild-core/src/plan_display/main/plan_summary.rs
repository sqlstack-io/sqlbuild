//! A plan document's summary: its facts, and each section as a count with its notes, never its rows.

use serde_json::Value;

use crate::plan_display::_helpers::plan_pages::PlanRows;
use crate::plan_display::_helpers::plan_sections::{emptied, section_counts};

/// The document with every row list emptied and each section counted; a failure report is returned unchanged.
pub fn plan_summary(document: &Value) -> Value {
    let Some(fields) = document
        .as_object()
        .filter(|fields| fields.contains_key("models"))
    else {
        return document.clone();
    };
    let (sections, pages) = section_counts(&PlanRows::new(document));
    let mut summary = emptied(fields);
    summary.insert("sections".into(), Value::Array(sections));
    summary.insert("pages".into(), Value::Object(pages));
    Value::Object(summary)
}
