//! Declared unique keys of project relations, derived natively from their key facts.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;
use sqlbuild_discovery::project_validation::main::at_most_one_csv_data_row::at_most_one_csv_data_row;
use sqlbuild_discovery::project_validation::models::CsvSettings;
use sqlbuild_rules::relation_keys::main::declared_relation_keys::declared_relation_keys as derive_keys;
use sqlbuild_rules::relation_keys::models::{DeclaredKeyColumn, DeclaredRelation};

use crate::bindings::_helpers::compiled_project::schema_facts::python_column_facts;
use crate::bindings::_helpers::compiled_views::compiled_model_view::NativeCompiledModel;
use crate::bindings::_helpers::compiled_views::model_input_view::{
    NativeModelInput, model_relation_key_row,
};
use crate::bindings::_helpers::compiled_views::source_views::{
    SourceKeyFacts, SourceViews, read_source_view,
};
use crate::bindings::_helpers::discovery::project_facts::seed_csv_settings;
use crate::bindings::_helpers::records::fields::field as native_field;

const MATERIALIZED_CONFIG: &str = "materialized";
const INCREMENTAL_MODE_CONFIG: &str = "incremental_mode";
const INCREMENTAL_STRATEGY_CONFIG: &str = "incremental_strategy";
const CURSOR_CONFIG: &str = "cursor";
const UNIQUE_KEY_CONFIG: &str = "unique_key";
const UNIQUE_AUDIT_COLUMN_ARGUMENT: &str = "column";
const RELATION_KIND_SEED: &str = "seed";
const RELATION_KIND_SOURCE: &str = "source";

/// Native declared columns and model audits used to derive relation keys.
type DeclaredKeyFacts<'a> = (&'a [SchemaColumnFact], Vec<(String, Option<String>)>);

positional_record! {
    extract, into;
    /// `(kind, name, materialized, mode, strategy, cursor, unique key, columns, audits)`.
    pub(crate) struct RelationKeyRow {
        kind: String,
        name: String,
        materialized: String,
        mode: String,
        strategy: String,
        has_cursor: bool,
        unique_key: Option<Vec<String>>,
        columns: Vec<(String, Vec<String>, bool)>,
        audits: Vec<(String, Option<String>)>,
    }
}

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

/// Each audit spec's `(definition name, column argument when it is text)`.
pub(crate) fn audit_key_rows(audits: &Bound<'_, PyAny>) -> PyResult<Vec<(String, Option<String>)>> {
    let mut rows: Vec<(String, Option<String>)> = Vec::new();
    for audit in audits.try_iter()? {
        let audit: Bound<'_, PyAny> = audit?;
        let argument: Option<Bound<'_, PyAny>> = native_field(&audit, "arguments")?
            .downcast::<PyDict>()?
            .get_item(UNIQUE_AUDIT_COLUMN_ARGUMENT)?;
        rows.push((
            native_field(&audit, "definition_name")?.extract()?,
            argument
                .filter(pyo3::types::PyAnyMethods::is_instance_of::<PyString>)
                .map(|value| value.extract())
                .transpose()?,
        ));
    }
    Ok(rows)
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

/// What `relation_keys` reads for a source: its native view itself, or its `source_entry`.
pub(crate) fn source_key_input<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if read_source_view(source, |_, _| Ok(())).is_some() {
        return Ok(source.clone());
    }
    native_field(source, "source_entry")
}

/// `str(values.get(key, ""))`.
fn config_text(values: &Bound<'_, PyDict>, key: &str) -> PyResult<String> {
    match values.get_item(key)? {
        Some(value) => value.str()?.extract(),
        None => Ok(String::new()),
    }
}

/// `unique_key` as a name list, when it is a name or a non-empty list or tuple of names.
fn unique_key(values: &Bound<'_, PyDict>) -> PyResult<Option<Vec<String>>> {
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

/// The key row of a seed, source or hand-built model from its config values and schema entry.
pub(crate) fn entry_relation_key_row(
    kind: &str,
    name: String,
    values: &Bound<'_, PyDict>,
    entry: &Bound<'_, PyAny>,
) -> PyResult<RelationKeyRow> {
    if entry.is_none() {
        return relation_key_row(kind, name, values, None);
    }
    let columns: Vec<SchemaColumnFact> =
        python_column_facts(&declared_specs(entry, "columns")?, None)?;
    let audits: Bound<'_, PyAny> = declared_specs(entry, "audits")?;
    relation_key_row(
        kind,
        name,
        values,
        Some((&columns, audit_key_rows(&audits)?)),
    )
}

/// `getattr(entry, name, None) or ()`.
fn declared_specs<'py>(entry: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    match entry.getattr_opt(name)? {
        Some(specs) if specs.is_truthy()? => Ok(specs),
        _ => Ok(PyTuple::empty(entry.py()).into_any()),
    }
}

/// Declared keys by `kind:name` identity, in identity order.
pub(crate) type RelationKeys = Vec<(String, Vec<Vec<String>>)>;

/// Names of the seeds whose CSV holds at most one data row, from `seed_file` and `schema_entry`.
pub(crate) fn one_row_seed_names(seeds: &[Bound<'_, PyAny>]) -> PyResult<Vec<String>> {
    let mut candidates: Vec<(String, PathBuf, CsvSettings)> = Vec::with_capacity(seeds.len());
    for seed in seeds {
        let entry: Bound<'_, PyAny> = native_field(seed, "schema_entry")?;
        candidates.push((
            native_field(&entry, "name")?.extract()?,
            native_field(&native_field(seed, "seed_file")?, "file_path")?.extract()?,
            seed_csv_settings(&entry)?,
        ));
    }
    let py: Python<'_> = match seeds.first() {
        Some(seed) => seed.py(),
        None => return Ok(Vec::new()),
    };
    Ok(py.detach(|| {
        candidates
            .into_iter()
            .filter(|(_, path, settings)| at_most_one_csv_data_row(path, settings))
            .map(|(name, _, _)| name)
            .collect()
    }))
}

/// Keys proving one row per key by `kind:name`; seeds in `one_row_seeds` also get the empty key.
pub(crate) fn relation_keys<'py>(
    py: Python<'py>,
    mut rows: Vec<RelationKeyRow>,
    (seeds, sources): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    one_row_seeds: &[String],
) -> PyResult<RelationKeys> {
    for (kind, entries) in [(RELATION_KIND_SEED, seeds), (RELATION_KIND_SOURCE, sources)] {
        for entry in entries.try_iter()? {
            let entry: Bound<'py, PyAny> = entry?;
            if let Some(row) = source_view_key_row(&entry) {
                rows.push(row?);
                continue;
            }
            let name: String = native_field(&entry, "name")?.extract()?;
            rows.push(entry_relation_key_row(
                kind,
                name,
                &PyDict::new(py),
                &entry,
            )?);
        }
    }
    let mut relations: Vec<DeclaredRelation> = rows.into_iter().map(declared_relation).collect();
    for relation in &mut relations {
        relation.at_most_one_row =
            relation.kind == RELATION_KIND_SEED && one_row_seeds.contains(&relation.name);
    }
    let keys: BTreeMap<String, Vec<Vec<String>>> = py.detach(|| derive_keys(&relations));
    Ok(keys.into_iter().collect())
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
fn declared_relation_keys<'py>(
    py: Python<'py>,
    model_rows: Vec<RelationKeyRow>,
    seeds: &Bound<'py, PyAny>,
    sources: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    keys_dict(py, relation_keys(py, model_rows, (seeds, sources), &[])?)
}

/// Declared keys of a compiled project's models, seeds and sources.
pub(crate) fn project_relation_keys(project: &Bound<'_, PyAny>) -> PyResult<RelationKeys> {
    let py: Python<'_> = project.py();
    let mut rows: Vec<RelationKeyRow> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        rows.push(model_key_row(&model?)?);
    }
    let compiled_seeds: Vec<Bound<'_, PyAny>> = native_field(project, "seeds")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let one_row_seeds: Vec<String> = one_row_seed_names(&compiled_seeds)?;
    let mut seeds: Vec<Bound<'_, PyAny>> = Vec::new();
    for seed in &compiled_seeds {
        seeds.push(native_field(seed, "schema_entry")?);
    }
    let mut sources: Vec<Bound<'_, PyAny>> = Vec::new();
    for source in native_field(project, "sources")?.try_iter()? {
        sources.push(source_key_input(&source?)?);
    }
    relation_keys(
        py,
        rows,
        (
            PyTuple::new(py, seeds)?.as_any(),
            PyTuple::new(py, sources)?.as_any(),
        ),
        &one_row_seeds,
    )
}

/// Declared keys of a compiled project's models, seeds and sources.
#[pyfunction]
fn compiled_relation_keys<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    keys_dict(project.py(), project_relation_keys(project)?)
}

fn declared_relation(row: RelationKeyRow) -> DeclaredRelation {
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(declared_relation_keys, module)?)?;
    module.add_function(wrap_pyfunction!(compiled_relation_keys, module)?)
}

/// The declared key facts of a model input or compiled model.
pub(crate) fn model_key_row(model: &Bound<'_, PyAny>) -> PyResult<RelationKeyRow> {
    let py: Python<'_> = model.py();
    if let Ok(input) = model.downcast::<NativeModelInput>() {
        return input.get().read_with_owner(py, |owner, record| {
            model_relation_key_row(owner, record, py)
        });
    }
    if let Ok(compiled) = model.downcast::<NativeCompiledModel>() {
        return compiled.get().relation_key_row(py);
    }
    native_field(model, "relation_key_row")?.extract()
}
