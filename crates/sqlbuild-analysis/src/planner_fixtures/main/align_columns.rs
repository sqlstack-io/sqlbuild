//! Align supplied columns to their authoritative schema without rendering adapter SQL.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::planner_fixtures::_helpers::schema_order::supplied_in_schema_order;
use crate::planner_fixtures::models::{Alignment, AlignmentInput, RequiredColumn};

/// Resolve missing columns, diagnostics and star-prefix ordering for one fixture.
pub fn align_columns(label: &str, input: AlignmentInput<'_>) -> Alignment {
    let missing = missing_columns(&input);
    if missing.is_empty() && !input.typed_nulls {
        return Alignment::default();
    }
    let metadata: HashMap<&str, usize> = input
        .metadata
        .iter()
        .enumerate()
        .map(|(index, column)| (column.folded_name.as_str(), index))
        .collect();
    let mut unknown: Vec<&str> = Vec::new();
    let mut non_nullable: Vec<&str> = Vec::new();
    let mut readers: BTreeSet<&str> = BTreeSet::new();
    let mut completed: Vec<usize> = Vec::new();
    for required in missing {
        match metadata.get(required.folded_name.as_str()).copied() {
            Some(index) if input.metadata[index].sql_type.is_some() => {
                if input.metadata[index].nullable == Some(false) {
                    non_nullable.push(required.name.as_str());
                } else {
                    completed.push(index);
                }
            }
            _ => {
                unknown.push(required.name.as_str());
                readers.extend(required.readers.iter().map(String::as_str));
            }
        }
    }
    let diagnostics = missing_diagnostics(label, &unknown, &non_nullable, readers);
    if !diagnostics.is_empty() {
        return Alignment {
            diagnostics,
            completed: None,
        };
    }
    if input.star && !completed.is_empty() && !supplied_in_schema_order(&input) {
        return Alignment {
            diagnostics: vec![format!(
                "{label} supplies columns out of authoritative order for a SELECT * closure; provide the leading relation columns in schema order"
            )],
            completed: None,
        };
    }
    completed.sort_unstable();
    completed.dedup();
    Alignment {
        diagnostics,
        completed: Some(completed),
    }
}

fn missing_columns<'a>(input: &AlignmentInput<'a>) -> Vec<&'a RequiredColumn> {
    let supplied: HashSet<&str> = input
        .supplied
        .iter()
        .map(|column| column.folded_name.as_str())
        .collect();
    let mut missing: Vec<_> = input
        .required
        .iter()
        .filter(|column| !supplied.contains(column.folded_name.as_str()))
        .collect();
    missing.sort_by(|left, right| left.name.cmp(&right.name));
    missing
}

fn missing_diagnostics(
    label: &str,
    unknown: &[&str],
    non_nullable: &[&str],
    readers: BTreeSet<&str>,
) -> Vec<String> {
    let mut diagnostics: Vec<String> = Vec::new();
    if !unknown.is_empty() {
        diagnostics.push(format!(
            "{label} is missing required columns: {}; provide explicit values because their types are not authoritative (read by: {})",
            unknown.join(", "), readers.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if !non_nullable.is_empty() {
        diagnostics.push(format!(
            "{label} must provide required non-nullable columns: {}",
            non_nullable.join(", ")
        ));
    }
    diagnostics
}
