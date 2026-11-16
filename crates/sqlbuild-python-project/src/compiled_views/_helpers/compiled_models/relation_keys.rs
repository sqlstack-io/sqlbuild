//! Declared unique keys of project relations, derived natively from their key facts.

use crate::compiled_views::main::compiled_models::project_relation_keys::project_relation_keys;
use crate::compiled_views::main::compiled_models::relation_keys::relation_keys;
use crate::compiled_views::models::RelationKeyRow;
use crate::compiled_views::types::RelationKeys;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyString, PyTuple};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;
use sqlbuild_rules::relation_keys::models::{DeclaredKeyColumn, DeclaredRelation};

use crate::resource_views::main::source_views::read_source_view::read_source_view;
use crate::resource_views::models::SourceKeyFacts;
use crate::resource_views::types::SourceViews;

const MATERIALIZED_CONFIG: &str = "materialized";
const INCREMENTAL_MODE_CONFIG: &str = "incremental_mode";
const INCREMENTAL_STRATEGY_CONFIG: &str = "incremental_strategy";
const CURSOR_CONFIG: &str = "cursor";
const UNIQUE_KEY_CONFIG: &str = "unique_key";
pub(crate) const UNIQUE_AUDIT_COLUMN_ARGUMENT: &str = "column";
pub(crate) const RELATION_KIND_SEED: &str = "seed";
pub(crate) const RELATION_KIND_SOURCE: &str = "source";

/// Native declared columns and model audits used to derive relation keys.
type DeclaredKeyFacts<'a> = (&'a [SchemaColumnFact], Vec<(String, Option<String>)>);

/// The key row of one relation from its config values and declared column and audit specs.
pub(crate) fn relation_key_row(
    kind: &str,
    name: String,
    values: &Bound<'_, PyDict>,
    declared: Option<DeclaredKeyFacts<'_>>,
) -> PyResult<RelationKeyRow> {
    let mut columns: Vec<(String, Vec<String>, bool)> = Vec::new();
    let mut audits: Vec<(String, Option<String>)> = Vec::new();
    if let Some((declared_columns, declared_audits)) = declared {
        for column in declared_columns {
            columns.push((
                column.name.clone(),
                column.audit_definitions.clone(),
                column.nullable == Some(false),
            ));
        }
        audits = declared_audits;
    }
    Ok(RelationKeyRow {
        kind: kind.to_owned(),
        name,
        materialized: config_text(values, MATERIALIZED_CONFIG)?,
        mode: config_text(values, INCREMENTAL_MODE_CONFIG)?,
        strategy: config_text(values, INCREMENTAL_STRATEGY_CONFIG)?,
        has_cursor: values
            .get_item(CURSOR_CONFIG)?
            .is_some_and(|value| value.is_truthy().unwrap_or(false)),
        unique_key: unique_key(values)?,
        columns,
        audits,
    })
}

/// A source's key row from its native view's declared facts, or `None` for any other object.
pub(crate) fn source_view_key_row(view: &Bound<'_, PyAny>) -> Option<PyResult<RelationKeyRow>> {
    read_source_view(view, |source, py| {
        let facts: &SourceKeyFacts = source.key_facts(py)?;
        Ok(RelationKeyRow {
            kind: RELATION_KIND_SOURCE.to_owned(),
            name: source.attached().name.clone(),
            materialized: String::new(),
            mode: String::new(),
            strategy: String::new(),
            has_cursor: false,
            unique_key: None,
            columns: facts
                .columns
                .iter()
                .map(|column| {
                    (
                        column.name.clone(),
                        column.audit_definitions.clone(),
                        column.nullable == Some(false),
                    )
                })
                .collect(),
            audits: facts.audits.clone(),
        })
    })
}

/// `str(values.get(key, ""))`.
fn config_text(values: &Bound<'_, PyDict>, key: &str) -> PyResult<String> {
    match values.get_item(key)? {
        Some(value) => value.str()?.extract(),
        None => Ok(String::new()),
    }
}

/// `unique_key` as a name list, when it is a name or a non-empty list or tuple of names.
pub(crate) fn unique_key(values: &Bound<'_, PyDict>) -> PyResult<Option<Vec<String>>> {
    let Some(value) = values.get_item(UNIQUE_KEY_CONFIG)? else {
        return Ok(None);
    };
    if value.is_instance_of::<PyString>() {
        return Ok(Some(vec![value.extract()?]));
    }
    if !(value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()) {
        return Ok(None);
    }
    let mut names: Vec<String> = Vec::new();
    for item in value.try_iter()? {
        let item: Bound<'_, PyAny> = item?;
        if !item.is_instance_of::<PyString>() {
            return Ok(None);
        }
        names.push(item.str()?.extract()?);
    }
    Ok(Some(names))
}

/// `getattr(entry, name, None) or ()`.
pub(crate) fn declared_specs<'py>(
    entry: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    match entry.getattr_opt(name)? {
        Some(specs) if specs.is_truthy()? => Ok(specs),
        _ => Ok(PyTuple::empty(entry.py()).into_any()),
    }
}

/// `keys` as a `{identity: ((column, ...), ...)}` dict.
fn keys_dict<'py>(py: Python<'py>, keys: RelationKeys) -> PyResult<Bound<'py, PyDict>> {
    let found: Bound<'py, PyDict> = PyDict::new(py);
    for (identity, unique_keys) in keys {
        let mut tuples: Vec<Bound<'py, PyTuple>> = Vec::with_capacity(unique_keys.len());
        for key in unique_keys {
            tuples.push(PyTuple::new(py, key)?);
        }
        found.set_item(identity, PyTuple::new(py, tuples)?)?;
    }
    Ok(found)
}

/// Keys that prove one row per key, by `kind:name` identity, from model rows, seeds and sources.
#[pyfunction]
#[pyo3(signature = (*, model_rows, seeds, sources))]
pub(crate) fn declared_relation_keys<'py>(
    py: Python<'py>,
    model_rows: Vec<RelationKeyRow>,
    seeds: &Bound<'py, PyAny>,
    sources: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    keys_dict(py, relation_keys(py, model_rows, (seeds, sources), &[])?)
}

/// Declared keys of a compiled project's models, seeds and sources.
#[pyfunction]
pub(crate) fn compiled_relation_keys<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    keys_dict(project.py(), project_relation_keys(project)?)
}

pub(crate) fn declared_relation(row: RelationKeyRow) -> DeclaredRelation {
    let RelationKeyRow {
        kind,
        name,
        materialized,
        mode,
        strategy,
        has_cursor,
        unique_key,
        columns,
        audits,
    } = row;
    DeclaredRelation {
        kind,
        name,
        materialized,
        incremental_mode: mode,
        incremental_strategy: strategy,
        has_cursor,
        unique_key,
        columns: columns
            .into_iter()
            .map(|(name, audits, not_nullable)| DeclaredKeyColumn {
                name,
                audits,
                not_nullable,
            })
            .collect(),
        audits,
        at_most_one_row: false,
    }
}
