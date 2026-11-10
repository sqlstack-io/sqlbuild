//! The downstream columns a change reaches, from column lineage.

use sqlbuild_analysis::review_command::constants::NO_COMPARISON_MODE;
use std::collections::{BTreeMap, BTreeSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyFrozenSet, PyString};
use sqlbuild_analysis::review_command::models::NodeComparison;

use crate::bindings::_helpers::compiled_views::{model_reads, project_reads};
use crate::bindings::_helpers::lineage::column_lineage_graph::ProjectColumnLineage;
use crate::bindings::_helpers::lineage::project_lineage::build_project_column_lineage;
use crate::bindings::_helpers::records::construct::enum_member;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

/// The columns of a changed model a comparison proved changed, or `None` when every column may.
fn changed_columns(comparison: Option<&NodeComparison>) -> Option<Vec<String>> {
    let comparison: &NodeComparison = comparison?;
    if !comparison.complete || comparison.mode == NO_COMPARISON_MODE {
        return None;
    }
    let mut columns: Vec<String> = comparison.added_columns.clone();
    columns.extend(
        comparison
            .type_changes
            .iter()
            .map(|(column, _, _)| column.clone()),
    );
    columns.extend(
        comparison
            .columns
            .iter()
            .map(|column| column.column.clone()),
    );
    Some(columns)
}

/// Affected columns of each downstream node of `changed`, within `affected`.
pub(crate) fn affected_columns(
    (project, dialect): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (changed, affected): (&[String], &BTreeSet<String>),
    comparisons: &BTreeMap<String, NodeComparison>,
) -> PyResult<BTreeMap<String, Vec<String>>> {
    let py: Python<'_> = project.py();
    let mut models: BTreeMap<String, Bound<'_, PyAny>> = BTreeMap::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        let name: String = model_reads::model_name(&model)?;
        if affected.contains(&name) {
            models.insert(name, model);
        }
    }
    let names: Vec<&String> = models.keys().collect();
    let lineage = build_project_column_lineage(
        project,
        Some(dialect),
        &enum_member(py, RecordId::ColumnLineageMode, "FAST")?,
        Some(PyFrozenSet::new(py, &names)?.as_any()),
    );
    let Ok(Some(lineage)) = lineage else {
        return Ok(BTreeMap::new());
    };
    let lineage: Bound<'_, PyAny> = lineage.into_bound(py);
    let Ok(lineage) = lineage.cast::<ProjectColumnLineage>() else {
        return Ok(BTreeMap::new());
    };
    let mut pending: Vec<(String, String)> = Vec::new();
    for name in changed {
        let Some(model) = models.get(name) else {
            continue;
        };
        let columns: Vec<String> = match changed_columns(comparisons.get(name)) {
            Some(columns) => columns,
            None => model_reads::inferred_names(model)?.unwrap_or_default(),
        };
        pending.extend(columns.into_iter().map(|column| (name.clone(), column)));
    }
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut reached: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    while let Some((resource, column)) = pending.pop() {
        if !seen.insert((resource.clone(), column.clone())) {
            continue;
        }
        let column_name: Bound<'_, PyAny> = PyString::new(py, &column).into_any();
        let Ok(edges) = ProjectColumnLineage::column_consumers(lineage, &resource, &column_name)
        else {
            continue;
        };
        for edge in edges.try_iter()? {
            let target: Bound<'_, PyAny> = native_field(&edge?, "target")?;
            let name: String = native_field(&target, "resource_name")?.extract()?;
            let column_name: String = native_field(&target, "column_name")?.extract()?;
            if !affected.contains(&name) || changed.contains(&name) {
                continue;
            }
            reached
                .entry(name.clone())
                .or_default()
                .insert(column_name.clone());
            pending.push((name, column_name));
        }
    }
    Ok(reached
        .into_iter()
        .map(|(name, columns)| (name, columns.into_iter().collect()))
        .collect())
}
