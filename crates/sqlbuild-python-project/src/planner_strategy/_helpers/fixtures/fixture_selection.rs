//! Fixture analysis selection and unknown-column diagnostics over the planning context.

use std::collections::HashSet;

use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDictMethods;
use sqlbuild_analysis::planner_fixtures::main::{
    fixture_schema::fixture_schema, requires_analysis::requires_analysis,
    unknown_columns::unknown_columns,
};

use crate::planner_strategy::_helpers::fixtures::fixture_preparation::PreparedFixtures;
use crate::planner_strategy::_helpers::fixtures::fixture_values::{columns, diagnostic, label};
use crate::planner_strategy::models::RelationFixturePlanningContext;
use crate::planner_strategy::types::FixtureKey;

/// Fixtures whose inferred columns do not already match an authoritative relation shape.
pub(crate) fn fixture_keys_requiring_analysis(
    prepared: &PreparedFixtures<'_>,
    context: &RelationFixturePlanningContext,
) -> PyResult<HashSet<FixtureKey>> {
    let mut selected: HashSet<FixtureKey> = HashSet::new();
    for (key, python_key) in &prepared.keys {
        let Some(inferred) = prepared.inferred.get_item(python_key)? else {
            continue;
        };
        if requires_analysis(
            &columns(&inferred, true)?,
            context.authoritative_columns.get(key),
        ) {
            selected.insert(key.clone());
        }
    }
    Ok(selected)
}

/// Diagnostics for fixtures that supply columns an authoritative relation does not have.
pub(crate) fn unknown_fixture_column_diagnostics<'py>(
    prepared: &PreparedFixtures<'py>,
    context: &RelationFixturePlanningContext,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::new();
    for (key, python_key) in &prepared.keys {
        let Some(inferred) = prepared.inferred.get_item(python_key)? else {
            continue;
        };
        let Some(relation) = context.relations.get(key) else {
            continue;
        };
        let schema = fixture_schema(&relation.columns, relation.authoritative);
        if let Some(message) = unknown_columns(
            &label(python_key)?,
            &columns(&inferred, true)?,
            schema.authoritative_names.as_ref(),
        ) {
            diagnostics.push(diagnostic(python_key, &message)?);
        }
    }
    Ok(diagnostics)
}
