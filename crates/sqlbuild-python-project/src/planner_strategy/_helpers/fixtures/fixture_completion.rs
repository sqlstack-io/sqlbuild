//! Relation fixture completion over the Rust fixture planning context.

use crate::planner_strategy::main::complete_fixtures::complete_fixtures;
use crate::planner_strategy::models::FixtureCompletion;
use std::collections::HashSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use sqlbuild_analysis::planner_fixtures::main::align_columns::align_columns;
use sqlbuild_analysis::planner_fixtures::models::{AlignmentInput, FixtureColumn};

use crate::planner_strategy::_helpers::fixtures::fixture_context::planning_context as project_planning_context;
use crate::planner_strategy::_helpers::fixtures::fixture_preparation::PreparedFixtures;
use crate::planner_strategy::_helpers::fixtures::fixture_selection::unknown_fixture_column_diagnostics;
use crate::planner_strategy::_helpers::fixtures::fixture_sql::{CompletionSql, render_completed};
use crate::planner_strategy::_helpers::fixtures::fixture_values::{
    columns, diagnostic, label, record,
};
use crate::planner_strategy::models::{ExpectedTypes, RelationFixturePlanningContext};
use crate::planner_strategy::types::FixtureKey;
use sqlbuild_python_base::records::types::RecordId;

impl ExpectedTypes<'_> {
    /// The relation's expected column types: the chain's propagated types, else the context's.
    pub(crate) fn get(&self, key: &FixtureKey) -> Option<&[(String, String)]> {
        self.overlay
            .iter()
            .find(|(known, _)| known == key)
            .map(|(_, types)| types.as_slice())
            .or_else(|| self.base.get(key).map(Vec::as_slice))
    }
}

pub(crate) fn complete_prepared<'py, 'a>(
    mut prepared: PreparedFixtures<'py>,
    (context, required): (
        &'a RelationFixturePlanningContext,
        &sqlbuild_analysis::planner_fixtures::models::FixtureReadRequirements,
    ),
    expected: ExpectedTypes<'a>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<FixtureCompletion<'py, 'a>> {
    let py = adapter.py();
    let python_keys: Vec<(FixtureKey, Bound<'py, PyAny>)> = prepared.keys.clone();
    let python_key = |key: &FixtureKey| -> Option<&Bound<'py, PyAny>> {
        python_keys
            .iter()
            .find(|(known, _)| known == key)
            .map(|(_, python)| python)
    };
    let typed_nulls = PyDict::new(py);
    for (key, original) in &python_keys {
        let Some(names) = prepared.null_names.get_item(original)? else {
            continue;
        };
        let types: &[(String, String)] = expected.get(key).unwrap_or_default();
        let selected = PyDict::new(py);
        for name in names.try_iter()? {
            let name = name?;
            let folded: String = name.extract()?;
            if let Some((_, sql_type)) = types.iter().find(|(column, _)| *column == folded) {
                selected.set_item(name, sql_type)?;
            }
        }
        if !selected.is_empty() {
            typed_nulls.set_item(original, selected)?;
        }
    }
    for value in unknown_fixture_column_diagnostics(&prepared, context)? {
        prepared.diagnostics.push(value);
    }
    let mut completed_keys: HashSet<FixtureKey> = HashSet::new();
    for (key, requirements) in &required.columns {
        let Some(original) = python_key(key) else {
            continue;
        };
        let Some(inferred) = prepared.inferred.get_item(original)? else {
            continue;
        };
        let typed = typed_nulls
            .get_item(original)?
            .map(|value| value.cast_into::<PyDict>())
            .transpose()?
            .unwrap_or_else(|| PyDict::new(py));
        let metadata: &[FixtureColumn] = context
            .relations
            .get(key)
            .map(|relation| relation.columns.as_slice())
            .unwrap_or_default();
        let alignment = align_columns(
            &label(original)?,
            AlignmentInput {
                supplied: &columns(&inferred, true)?,
                metadata,
                required: requirements,
                star: required.stars.contains(key),
                typed_nulls: !typed.is_empty(),
            },
        );
        for message in alignment.diagnostics {
            prepared.diagnostics.push(diagnostic(original, &message)?);
        }
        let Some(completed) = alignment.completed else {
            continue;
        };
        let completed: Vec<FixtureColumn> = completed
            .into_iter()
            .map(|index| metadata[index].clone())
            .collect();
        let sql = render_completed(
            CompletionSql {
                sql: &fixture_sql(&prepared, original)?,
                inferred,
                typed_nulls: typed,
                quoted_names: quoted_names(&prepared, original)?,
                completed,
            },
            adapter,
        )?;
        prepared.sql.set_item(original, sql)?;
        completed_keys.insert(key.clone());
    }
    for (key, original) in &python_keys {
        let Some(typed) = typed_nulls.get_item(original)? else {
            continue;
        };
        if completed_keys.contains(key) {
            continue;
        }
        let inferred = prepared
            .inferred
            .get_item(original)?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("fixture inference"))?;
        let sql = render_completed(
            CompletionSql {
                sql: &fixture_sql(&prepared, original)?,
                inferred,
                typed_nulls: typed.cast_into::<PyDict>()?,
                quoted_names: quoted_names(&prepared, original)?,
                completed: Vec::new(),
            },
            adapter,
        )?;
        prepared.sql.set_item(original, sql)?;
    }
    Ok(FixtureCompletion {
        sql_by_key: prepared.sql,
        inferred_by_fixture: prepared.inferred,
        expected_types: expected,
        diagnostics: prepared.diagnostics,
    })
}

fn fixture_sql(prepared: &PreparedFixtures<'_>, key: &Bound<'_, PyAny>) -> PyResult<String> {
    prepared
        .sql
        .get_item(key)?
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("fixture SQL"))?
        .extract()
}

fn quoted_names<'py>(
    prepared: &PreparedFixtures<'py>,
    key: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(prepared
        .quoted_names
        .get_item(key)?
        .unwrap_or(PyFrozenSet::empty(key.py())?.into_any()))
}

/// Complete one chain's fixture groups, building the planning context when none is given.
#[pyfunction]
#[pyo3(signature = (*, project, adapter, request, planning_context = None))]
pub(crate) fn build_relation_fixture_completion<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    request: (Vec<String>, Bound<'py, PyAny>),
    planning_context: Option<Bound<'py, RelationFixturePlanningContext>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let built: RelationFixturePlanningContext;
    let context: &RelationFixturePlanningContext = match &planning_context {
        Some(context) => context.get(),
        None => {
            built = project_planning_context(project, None)?;
            &built
        }
    };
    let completion = complete_fixtures(context, adapter, (&request.0, &request.1))?;
    let expected = PyDict::new(py);
    for (key, types) in &completion.expected_types.overlay {
        let columns = PyDict::new(py);
        for (name, sql_type) in types {
            columns.set_item(name, sql_type)?;
        }
        expected.set_item(key, columns)?;
    }
    let kwargs = PyDict::new(py);
    kwargs.set_item("fixture_sql_by_key", completion.sql_by_key)?;
    kwargs.set_item("inferred_by_fixture", completion.inferred_by_fixture)?;
    kwargs.set_item("expected_types", expected)?;
    kwargs.set_item("diagnostics", PyTuple::new(py, completion.diagnostics)?)?;
    record(py, RecordId::RelationFixtureCompletion, &kwargs)
}
