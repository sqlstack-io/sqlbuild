//! Boundary conversion for native fixture read analysis.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyFrozenSet;
use sqlbuild_analysis::planner_fixtures::main::prewarm_reads::prewarm_reads;
use sqlbuild_analysis::planner_fixtures::main::required_columns::required_columns;
use sqlbuild_analysis::planner_fixtures::main::star_keys::star_keys;
use sqlbuild_analysis::planner_fixtures::models::{
    FixtureReadKey, FixtureReadModel, FixtureReadReference, FixtureReadRequirements,
    FixtureReadSchemas, FixtureStarRelation,
};
use sqlbuild_analysis::required_columns::models::ColumnRead;
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::native_host::types::Syntax;
use std::collections::{HashMap, HashSet};

use crate::compiled_views::main::compiled_models::dep_rows::dep_rows;
use crate::compiled_views::main::compiled_models::lineage_facts::lineage_facts;
use crate::compiled_views::main::compiled_models::proven_inputs::proven_inputs;
use crate::compiled_views::main::compiled_models::query_sql::query_sql;
use crate::compiled_views::main::compiled_models::star_flags::star_flags;
use crate::compiled_views::models::LineageFactRow;
use crate::planner_strategy::_helpers::fixtures::fixture_context::work_counts;
use crate::planner_strategy::constants::HIGH_CONFIDENCE;
use crate::planner_strategy::models::RelationFixturePlanningContext;
use crate::planner_strategy::types::FixtureKey;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::types::CompilerDetach;

/// The columns each fixture must supply for the chain's models, and the fixtures read through a star.
pub(crate) fn required_fixture_columns(
    context: &RelationFixturePlanningContext,
    ordered_model_names: &[String],
    fixture_keys: &HashSet<FixtureKey>,
    dialect: &Bound<'_, Dialect>,
) -> PyResult<FixtureReadRequirements> {
    let py = dialect.py();
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
    let analysis_dialect = analysis_dialect(dialect)?;
    let mut models: Vec<FixtureReadModel> = Vec::new();
    for name in ordered_model_names {
        let Some(model) = context.models_by_name.get(name) else {
            continue;
        };
        let model = model.bind(py);
        let Some(facts) = context.model_facts(py, name)? else {
            continue;
        };
        models.push(FixtureReadModel {
            name: name.clone(),
            key: read_key(&model, analysis_dialect.clone())?,
            high_confidence: facts.reads.clone(),
            stars: model_stars(&model, &schema, dialect)?,
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

/// Resolve, in parallel, the reads of every listed model that per-test planning can look up.
pub(crate) fn prewarm_fixture_reads(
    context: &RelationFixturePlanningContext,
    model_names: &[String],
    dialect: &Bound<'_, Dialect>,
) -> PyResult<()> {
    let py = dialect.py();
    let analysis_dialect = analysis_dialect(dialect)?;
    let mut keys: Vec<FixtureReadKey> = Vec::new();
    for name in model_names {
        if let Some(model) = context.models_by_name.get(name) {
            let model = model.bind(py);
            let key = read_key(&model, analysis_dialect.clone())?;
            if !key.references.is_empty() {
                keys.push(key);
            }
        }
    }
    let owner = work_counts(py)?;
    let mut owner = owner.borrow_mut();
    let cache = std::mem::take(&mut owner.cache);
    owner.cache = py
        .compiler_detach(|| Ok(prewarm_reads(cache, &keys)))
        .unwrap_or_default();
    Ok(())
}

fn analysis_dialect(dialect: &Bound<'_, Dialect>) -> PyResult<Option<String>> {
    dialect
        .syntax("expression_inference_profile")?
        .call0()?
        .getattr("sql_analysis_dialect")?
        .extract()
}

/// The read cache key of one model: its SQL, complete references and analysis dialect.
fn read_key(model: &Bound<'_, PyAny>, dialect: Option<String>) -> PyResult<FixtureReadKey> {
    let mut references: Vec<FixtureReadReference> = Vec::new();
    for reference in
        crate::compiled_views::main::compiled_models::references::references(model)?.try_iter()?
    {
        let reference = reference?;
        references.push(FixtureReadReference {
            kind: native_field(&reference, "ref_kind")?.extract()?,
            name: native_field(&reference, "ref_name")?.extract()?,
            package: native_field(&reference, "ref_package")?.extract()?,
            argument_count: native_field(&reference, "call_argument_count")?.extract()?,
        });
    }
    Ok(FixtureReadKey {
        sql: query_sql(model)?.extract()?,
        references,
        dialect,
    })
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
    dialect: &Bound<'_, Dialect>,
) -> PyResult<Vec<(String, String)>> {
    if !star_flags(model)?.0 {
        return Ok(Vec::new());
    }
    let mut deps: Vec<(String, String)> = Vec::new();
    for key in dep_rows(model)? {
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
            dialect
                .syntax("expression_inference_profile")?
                .call0()?
                .getattr("sql_analysis_dialect")?
                .extract()?
        } else {
            None
        };
    let sql: String = query_sql(model)?.extract()?;
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

/// Report resolver misses and retained entries, optionally clearing prior fixture reuse.
#[pyfunction]
#[pyo3(signature = (*, reset = false))]
pub(crate) fn fixture_read_cache_info(py: pyo3::Python<'_>, reset: bool) -> PyResult<(u64, usize)> {
    let owner = work_counts(py)?;
    let mut owner = owner.borrow_mut();
    if reset {
        owner.cache = Default::default();
    }
    Ok((owner.cache.misses, owner.cache.entries.len()))
}
