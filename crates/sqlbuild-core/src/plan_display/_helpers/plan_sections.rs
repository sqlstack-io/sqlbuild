//! A summary's section counts and a page's rows, read from a plan document's sections.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value, json};

use crate::plan_display::_helpers::diff_lines::config_change;
use crate::plan_display::_helpers::plan_pages::{
    Note, PlanRows, SECTIONS, lines, model_rows, node_key, row_diff_stats, row_name, text,
    warnings_about,
};
use crate::plan_display::_helpers::warning_groups::{group_json, warning_groups};
use crate::plan_display::constants::{
    CHANGED_ROW_LISTS, CONFIG_CHANGE_FIELD, DIFF_STATS_FIELD, MAX_SECTION_NOTES,
    MAX_WARNING_GROUPS, METADATA_FIELD, MODEL_DIFF_FIELDS, PLAN_ROW_LISTS, WARNING_GROUPS_FIELD,
};
use crate::plan_display::models::PlanPageSection;

/// Each nonempty section with its size and most frequent notes, and the sizes by section name.
pub(crate) fn section_counts(plan: &PlanRows<'_>) -> (Vec<Value>, Map<String, Value>) {
    let mut sections: Vec<Value> = Vec::new();
    let mut pages: Map<String, Value> = Map::new();
    for (section, name) in SECTIONS {
        let rows = plan.rows(section);
        if rows.is_empty() {
            continue;
        }
        let mut counts: HashMap<Note, usize> = HashMap::new();
        for row in &rows {
            for note in plan.notes(section, row) {
                *counts.entry(note).or_default() += 1;
            }
        }
        pages.insert(name.to_owned(), json!(rows.len()));
        sections.push(json!({"section": name, "total": rows.len(), "notes": frequent(counts)}));
    }
    (sections, pages)
}

/// The most frequent notes, at most `MAX_SECTION_NOTES`, by count and then by kind and value.
fn frequent(counts: HashMap<Note, usize>) -> Vec<Value> {
    let mut notes: Vec<(Note, usize)> = counts.into_iter().collect();
    notes.sort_by(|(left, left_count), (right, right_count)| {
        right_count.cmp(left_count).then_with(|| left.cmp(right))
    });
    notes
        .into_iter()
        .take(MAX_SECTION_NOTES)
        .map(|((kind, value), count)| json!({"kind": kind, "value": value, "count": count}))
        .collect()
}

/// The document's fields without metadata, with every row list emptied.
pub(crate) fn emptied(fields: &Map<String, Value>) -> Map<String, Value> {
    let mut summary: Map<String, Value> = fields
        .iter()
        .filter(|(field, _)| field.as_str() != METADATA_FIELD)
        .map(|(field, value)| (field.clone(), value.clone()))
        .collect();
    for list in PLAN_ROW_LISTS {
        if summary.contains_key(list) {
            summary.insert(list.to_owned(), json!([]));
        }
    }
    if let Some(Value::Object(changed)) = summary.get_mut("changed") {
        for list in CHANGED_ROW_LISTS {
            changed.insert(list.to_owned(), json!([]));
        }
    }
    if let Some(Value::Object(copy)) = summary.get_mut("synced_copy") {
        copy.insert("cursor_window_starts".to_owned(), json!({}));
    }
    summary
}

/// The summary's largest warning groups, at most `MAX_WARNING_GROUPS`; the warnings section pages the rest after them.
pub(crate) fn with_warning_groups(
    plan: &PlanRows<'_>,
    mut summary: Map<String, Value>,
) -> Map<String, Value> {
    let groups: Vec<Value> = warning_groups(&plan.rows(PlanPageSection::Warnings))
        .iter()
        .take(MAX_WARNING_GROUPS)
        .map(group_json)
        .collect();
    summary.insert(WARNING_GROUPS_FIELD.to_owned(), Value::Array(groups));
    summary
}

/// The rows whose name contains `name`, ignoring case; every row without a name.
pub(crate) fn matching<'document>(
    section: PlanPageSection,
    rows: &[&'document Value],
    name: Option<&str>,
) -> Vec<&'document Value> {
    let Some(wanted) = name.map(str::to_lowercase).filter(|name| !name.is_empty()) else {
        return rows.to_vec();
    };
    let mut found: Vec<&Value> = Vec::new();
    for row in rows {
        if row_name(section, row).to_lowercase().contains(&wanted) {
            found.push(row);
        }
    }
    found
}

/// What a page's models and seeds bring beside them: `--changed` runs, warnings and cursor window starts.
#[derive(Default)]
pub(crate) struct PageExtras {
    pub(crate) runs: Vec<Value>,
    pub(crate) warnings: Vec<Value>,
    pub(crate) cursor_window_starts: Map<String, Value>,
}

/// The extras of a page of `section`; a page of other rows brings none.
pub(crate) fn page_extras(
    plan: &PlanRows<'_>,
    section: PlanPageSection,
    page: &[&Value],
) -> PageExtras {
    let kind = match section {
        PlanPageSection::FirstRunSeeds
        | PlanPageSection::ChangedSeeds
        | PlanPageSection::UnchangedSeeds => "seed",
        section if model_rows(section) => "model",
        _ => return PageExtras::default(),
    };
    let window_starts = plan
        .document
        .get("synced_copy")
        .and_then(|copy| copy.get("cursor_window_starts"));
    let mut extras = PageExtras::default();
    let mut names: HashSet<String> = HashSet::new();
    for row in page {
        let name = text(row, "name");
        extras
            .runs
            .extend(plan.run_of(&node_key(kind, row)).cloned());
        if let Some(start) = window_starts.and_then(|starts| starts.get(name)) {
            extras
                .cursor_window_starts
                .insert(name.to_owned(), start.clone());
        }
        names.insert(name.to_owned());
    }
    if model_rows(section) {
        extras.warnings = warnings_about(plan.document, &names)
            .into_iter()
            .cloned()
            .collect();
    }
    extras
}

/// A model row with its diff counts in place of its diff text; other rows as the document has them.
pub(crate) fn shown_row(section: PlanPageSection, row: &Value) -> Value {
    let (true, Some(fields)) = (model_rows(section), row.as_object()) else {
        return row.clone();
    };
    let stats = row_diff_stats(row);
    let mut shown: Map<String, Value> = fields
        .iter()
        .filter(|(field, _)| !MODEL_DIFF_FIELDS.contains(&field.as_str()))
        .map(|(field, value)| (field.clone(), value.clone()))
        .collect();
    shown.remove(DIFF_STATS_FIELD);
    if let Some(change) = lines(row, "config_diff").and_then(|diff| config_change(&diff)) {
        shown.insert(CONFIG_CHANGE_FIELD.to_owned(), Value::String(change));
    }
    if !stats.is_empty() {
        shown.insert(DIFF_STATS_FIELD.to_owned(), Value::Object(stats));
    }
    Value::Object(shown)
}
