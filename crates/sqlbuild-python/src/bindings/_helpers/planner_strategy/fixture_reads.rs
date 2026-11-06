//! Boundary conversion for native fixture read analysis.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::PyFrozenSet;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::planner_fixtures::main::required_columns::required_columns;
use sqlbuild_analysis::planner_fixtures::main::star_keys::star_keys;
use sqlbuild_analysis::planner_fixtures::models::{
    FixtureReadKey, FixtureReadModel, FixtureReadReference, FixtureReadRequirements,
    FixtureReadSchemas, FixtureStarRelation,
};
use sqlbuild_analysis::required_columns::models::ColumnRead;
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use std::collections::{HashMap, HashSet};

use crate::bindings::_helpers::boundary::panics::value_error;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::model_reads::{
    LineageFactRow, lineage_facts, proven_inputs, star_flags,
};
use crate::bindings::_helpers::planner_strategy::constants::HIGH_CONFIDENCE;
use crate::bindings::_helpers::planner_strategy::fixture_context::work_counts;
use crate::bindings::_helpers::planner_strategy::models::RelationFixturePlanningContext;
use crate::bindings::_helpers::planner_strategy::types::FixtureKey;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::types::CompilerDetach;

/// The columns each fixture must supply for the chain's models, and the fixtures read through a star.
pub(crate) fn required_fixture_columns(
    context: &RelationFixturePlanningContext,
    ordered_model_names: &[String],
    fixture_keys: &HashSet<FixtureKey>,
    adapter: &Bound<'_, PyAny>,
) -> PyResult<FixtureReadRequirements> {
    let py = adapter.py();
    let mut schema: Vec<FixtureStarRelation> = Vec::new();
    let mut authoritative: HashMap<FixtureKey, HashSet<String>> = HashMap::new();
    for key in fixture_keys {
        let relation = context.relations.get(key);
        schema.push(FixtureStarRelation {
            key: key.clone(),
            columns: relation
                .map(|relation| relation.columns.clone())
                .unwrap_or_default(),
            authoritative: relation.is_some_and(|relation| relation.authoritative),
        });
        if let Some(names) = context.authoritative_columns.get(key) {
            authoritative.insert(key.clone(), names.iter().cloned().collect());
        }
    }
    let dialect = adapter
        .call_method0("expression_inference_profile")?
        .getattr("sql_analysis_dialect")?
        .extract::<Option<String>>()?;
    let mut models: Vec<FixtureReadModel> = Vec::new();
    for name in ordered_model_names {
        let Some(model) = context.models_by_name.get(name) else {
            continue;
        };
        let model = model.bind(py);
        let Some(facts) = context.model_facts(py, name)? else {
            continue;
        };
        let mut references: Vec<FixtureReadReference> = Vec::new();
        for reference in model_reads::references(&model)?.try_iter()? {
            let reference = reference?;
            references.push(FixtureReadReference {
                kind: native_field(&reference, "ref_kind")?.extract()?,
                name: native_field(&reference, "ref_name")?.extract()?,
                package: native_field(&reference, "ref_package")?.extract()?,
                argument_count: native_field(&reference, "call_argument_count")?.extract()?,
            });
        }
        models.push(FixtureReadModel {
            name: name.clone(),
            key: FixtureReadKey {
                sql: model_reads::query_sql(&model)?.extract()?,
                references,
                dialect: dialect.clone(),
            },
            high_confidence: facts.reads.clone(),
            stars: model_stars(&model, &schema, adapter)?,
        });
    }
    let owner = work_counts(py)?;
    let mut owner = owner.borrow_mut();
    let cache = std::mem::take(&mut owner.cache);
    let (cache, result) = catch_compiler_panic(|| {
        Ok(required_columns(
            cache,
            &models,
            FixtureReadSchemas {
                keys: fixture_keys,
                authoritative: &authoritative,
                relations: &schema,
            },
        ))
    })
    .map_err(value_error)?;
    owner.cache = cache;
    result.map_err(|error| value_error(format!("{error:?}")))
}

pub(crate) fn lineage_reads(model: &Bound<'_, PyAny>) -> PyResult<Vec<ColumnRead>> {
    let mut reads: Vec<ColumnRead> = Vec::new();
    for LineageFactRow {
        confidence,
        sources,
        ..
    } in lineage_facts(model)?.unwrap_or_default()
    {
        if confidence != HIGH_CONFIDENCE {
            continue;
        }
        for (resource_type, resource_name, column_name) in sources {
            reads.push(ColumnRead {
                resource_type,
                resource_name,
                column_name,
            });
        }
    }
    Ok(reads)
}

fn model_stars(
    model: &Bound<'_, PyAny>,
    schema: &[FixtureStarRelation],
    adapter: &Bound<'_, PyAny>,
) -> PyResult<Vec<(String, String)>> {
    if !star_flags(model)?.0 {
        return Ok(Vec::new());
    }
    let mut deps: Vec<(String, String)> = Vec::new();
    for key in model_reads::dep_rows(model)? {
        if schema.iter().any(|relation| relation.key == key) {
            deps.push(key);
        }
    }
    let dependencies = PyFrozenSet::new(model.py(), deps)?;
    let mut selected: Vec<FixtureStarRelation> = Vec::new();
    for dependency in dependencies.try_iter()? {
        let dependency: (String, String) = dependency?.extract()?;
        if let Some(relation) = schema.iter().find(|relation| relation.key == dependency) {
            selected.push(FixtureStarRelation {
                key: relation.key.clone(),
                columns: relation.columns.clone(),
                authoritative: relation.authoritative,
            });
        }
    }
    let inputs: Option<Vec<String>> = proven_inputs(model)?;
    let dialect: Option<String> =
        if inputs.is_none() && selected.iter().any(|relation| !relation.columns.is_empty()) {
            adapter
                .call_method0("expression_inference_profile")?
                .getattr("sql_analysis_dialect")?
                .extract()?
        } else {
            None
        };
    let sql: String = model_reads::query_sql(model)?.extract()?;
    model
        .py()
        .compiler_detach(|| {
            Ok(star_keys(
                &sql,
                &selected,
                dialect.as_deref(),
                inputs.as_deref(),
            ))
        })
        .map_err(value_error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(fixture_read_cache_info, module)?)
}

/// Report resolver misses and retained entries, optionally clearing prior fixture reuse.
#[pyfunction]
#[pyo3(signature = (*, reset = false))]
fn fixture_read_cache_info(py: pyo3::Python<'_>, reset: bool) -> PyResult<(u64, usize)> {
    let owner = work_counts(py)?;
    let mut owner = owner.borrow_mut();
    if reset {
        owner.cache = Default::default();
    }
    Ok((owner.cache.misses, owner.cache.entries.len()))
}
