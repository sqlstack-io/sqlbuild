//! Expected fixture type propagation boundary conversions.

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::planner_fixtures::main::propagate_types::propagate_types;
use sqlbuild_analysis::planner_fixtures::models::{
    FixtureTypeModel, FixtureTypeProjection, FixtureTypeRead, FixtureTypeState,
};

use sqlbuild_core::text::main::python_casefold::python_casefold;

use crate::compiled_views::main::compiled_models::lineage_facts::lineage_facts;
use crate::compiled_views::models::LineageFactRow;
use crate::planner_strategy::constants::DIRECT_TRANSFORM;
use crate::planner_strategy::constants::MODEL_REFERENCE_KIND;
use crate::planner_strategy::models::ExpectedTypes;
use crate::planner_strategy::models::RelationFixturePlanningContext;
use crate::planner_strategy::types::FixtureKey;

/// Each chain model's known column types with direct lineage types propagated from its inputs.
pub(crate) fn expected_fixture_types<'a>(
    context: &'a RelationFixturePlanningContext,
    py: Python<'_>,
    ordered_model_names: &[String],
) -> PyResult<ExpectedTypes<'a>> {
    let base = &context.expected_types;
    let mut state = FixtureTypeState::default();
    let mut overlay: Vec<(FixtureKey, Vec<(String, String)>)> = Vec::new();
    let mut models: Vec<FixtureTypeModel> = Vec::new();
    for name in ordered_model_names {
        let Some(facts) = context.model_facts(py, name)? else {
            continue;
        };
        let key: FixtureKey = (MODEL_REFERENCE_KIND.to_owned(), name.clone());
        let types = base
            .get(&key)
            .ok_or_else(|| PyKeyError::new_err(key.clone()))?
            .clone();
        state
            .known
            .insert(key.clone(), types.iter().cloned().collect());
        overlay.push((key.clone(), types));
        models.push(FixtureTypeModel {
            key,
            projections: facts.projections.clone(),
        });
    }
    for model in &models {
        for projection in &model.projections {
            for read in &projection.upstream {
                if state
                    .known
                    .get(&read.key)
                    .is_some_and(|types| types.contains_key(&read.column))
                {
                    continue;
                }
                if let Some(sql_type) = base.get(&read.key).and_then(|types| {
                    types
                        .iter()
                        .find(|(name, _)| *name == read.column)
                        .map(|(_, sql_type)| sql_type.clone())
                }) {
                    state
                        .known
                        .entry(read.key.clone())
                        .or_default()
                        .insert(read.column.clone(), sql_type);
                }
            }
        }
    }
    let state = propagate_types(state, &models);
    for (read, sql_type) in state.additions {
        let types = overlay
            .iter_mut()
            .find(|(key, _)| *key == read.key)
            .map(|(_, types)| types)
            .ok_or_else(|| PyKeyError::new_err("fixture types"))?;
        match types.iter_mut().find(|(name, _)| *name == read.column) {
            Some(existing) => existing.1 = sql_type,
            None => types.push((read.column, sql_type)),
        }
    }
    Ok(ExpectedTypes { overlay, base })
}

pub(crate) fn projections(model: &Bound<'_, PyAny>) -> PyResult<Vec<FixtureTypeProjection>> {
    let mut projections: Vec<FixtureTypeProjection> = Vec::new();
    for LineageFactRow {
        output_column,
        transform,
        sources,
        ..
    } in lineage_facts(model)?.unwrap_or_default()
    {
        if transform != DIRECT_TRANSFORM {
            continue;
        }
        let mut upstream: Vec<FixtureTypeRead> = Vec::with_capacity(sources.len());
        for (resource_type, resource_name, column_name) in sources {
            upstream.push(FixtureTypeRead {
                key: (resource_type, resource_name),
                column: python_casefold(&column_name),
            });
        }
        projections.push(FixtureTypeProjection {
            output_column: python_casefold(&output_column),
            upstream,
        });
    }
    Ok(projections)
}
