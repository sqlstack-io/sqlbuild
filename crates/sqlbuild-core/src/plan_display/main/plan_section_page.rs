//! One page of a plan document section the summary only counts.

use std::collections::HashSet;

use serde_json::Value;

use crate::plan_display::_helpers::plan_pages::{
    changed_runs, model_key, routine_models, run_node,
};
use crate::plan_display::models::{PlanPage, PlanPageSection};

/// Rows `offset..offset + limit` of `section` in plan order, with the section's size; a page of
/// routine models also carries their `--changed` runs.
pub fn plan_section_page(
    document: &Value,
    section: PlanPageSection,
    (offset, limit): (usize, usize),
) -> PlanPage {
    let rows: Vec<&Value> = match section {
        PlanPageSection::RoutineModels => routine_models(document, changed_runs(document).as_ref()),
        PlanPageSection::HeldBack => changed_section(document, "held_back"),
        PlanPageSection::Unchanged => changed_section(document, "unchanged"),
    };
    let page: Vec<Value> = rows
        .iter()
        .skip(offset)
        .take(limit)
        .map(|row| (*row).clone())
        .collect();
    let runs: Vec<Value> = if section == PlanPageSection::RoutineModels {
        let keys: HashSet<String> = page.iter().map(model_key).collect();
        changed_section(document, "runs")
            .into_iter()
            .filter(|run| run_node(run).is_some_and(|node| keys.contains(node)))
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    PlanPage {
        total: rows.len(),
        rows: page,
        runs,
    }
}

fn changed_section<'document>(document: &'document Value, section: &str) -> Vec<&'document Value> {
    document
        .get("changed")
        .and_then(|changed| changed.get(section))
        .and_then(Value::as_array)
        .map(|rows| rows.iter().collect())
        .unwrap_or_default()
}
