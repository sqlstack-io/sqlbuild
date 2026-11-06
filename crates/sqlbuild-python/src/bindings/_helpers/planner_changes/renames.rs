//! Model renames that downstream `__ref` names may have been rewritten for.

use crate::bindings::_helpers::fingerprints::query_hashes::compute_query_hash;
use crate::bindings::_helpers::records::construct::{enum_member, new_record_with};
use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PySet, PySetMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_analysis::planner_changes::main::origin_reference_names::origin_reference_names;
use sqlbuild_analysis::planner_changes::models::{NameSpan, RenameCandidate};
use sqlbuild_rules::lint_text::main::neutralize_interpolation::neutralize_interpolation;

use crate::bindings::_helpers::boundary::panics::value_error;
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, imported};
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::planner_identity::hashing::model_query_hash;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::RecordId;

const GENERIC_DIALECT: &str = "generic";
const FINAL_NEWLINE: &str = "\n";

const REF_CALL_PREFIX: &str = "__ref";

/// The rename mapping under which the model query hashes to its recorded definition.
pub(crate) fn origin_names(
    (query_sql, fingerprint): (String, &Bound<'_, PyAny>),
    renames: &Bound<'_, PyAny>,
    (recorded_hash, dialect): (&str, &Bound<'_, PyAny>),
) -> PyResult<Option<Vec<(String, String)>>> {
    let py: Python<'_> = fingerprint.py();
    let recorded_at: Bound<'_, PyAny> = utc(&native_field(fingerprint, "ts")?)?;
    let mut candidates: Vec<RenameCandidate> = Vec::new();
    for rename in renames.try_iter()? {
        let rename: Bound<'_, PyAny> = rename?;
        let renamed_at: Bound<'_, PyAny> = native_field(&rename, "recorded_at")?;
        candidates.push(RenameCandidate {
            new_name: native_field(&rename, "new_name")?.extract()?,
            origin_name: native_field(&rename, "origin_name")?.extract()?,
            applies: renamed_at.is_none() || recorded_at.lt(utc(&renamed_at)?)?,
        });
    }
    let dialect_name: String = dialect
        .extract::<Option<String>>()?
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| GENERIC_DIALECT.to_owned());
    let mut spans = |names: &BTreeSet<String>| ref_name_spans(&query_sql, &dialect_name, names);
    let dialect_option: Option<String> = dialect.extract()?;
    let mut query_hash = |text: &str| -> PyResult<String> {
        compute_query_hash(py, text, dialect_option.as_deref())
    };
    origin_reference_names(
        (&query_sql, recorded_hash),
        &candidates,
        (&mut spans, &mut query_hash),
    )
}

fn ref_name_spans(
    query_sql: &str,
    dialect: &str,
    names: &BTreeSet<String>,
) -> PyResult<Vec<NameSpan>> {
    let neutralized = neutralize_interpolation(query_sql, dialect, false).map_err(value_error)?;
    let mut spans: Vec<NameSpan> = Vec::new();
    for site in neutralized.sites {
        let Some((offset, name)) = ref_call_name(&site.original_text) else {
            continue;
        };
        if !names.contains(name) {
            continue;
        }
        let start: usize = site.original_start + site.original_text[..offset].chars().count();
        spans.push(NameSpan {
            start,
            end: start + name.chars().count(),
            name: name.to_owned(),
        });
    }
    Ok(spans)
}

/// The byte offset and name of a whole `__ref('name')` or `__ref("name")` call.
fn ref_call_name(text: &str) -> Option<(usize, &str)> {
    let rest: &str = text.strip_prefix(REF_CALL_PREFIX)?.trim_start();
    let rest: &str = rest.strip_prefix('(')?.trim_start();
    let quote: char = rest
        .chars()
        .next()
        .filter(|quote| matches!(quote, '\'' | '"'))?;
    let body: &str = &rest[1..];
    let length: usize = body.find(['\'', '"'])?;
    let name: &str = &body[..length];
    let tail: &str = body[length..].strip_prefix(quote)?.trim_start();
    let tail: &str = tail.strip_prefix(')')?;
    if name.is_empty() || !(tail.is_empty() || tail == FINAL_NEWLINE) {
        return None;
    }
    Some((text.len() - body.len(), name))
}

/// `value` in UTC; a naive value is taken as UTC.
fn utc<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let zone: Bound<'py, PyAny> = imported(py, "datetime", "UTC")?;
    if native_field(value, "tzinfo")?.is_none() || value.call_method0("utcoffset")?.is_none() {
        return call_with(&native_field(value, "replace")?, &[("tzinfo", zone)]);
    }
    value.call_method1("astimezone", (zone,))
}

/// The rename mapping under which `query_sql` hashes to the recorded definition, or `None`.
#[pyfunction(name = "origin_reference_names")]
#[pyo3(signature = (*, query_sql, recorded, renames, dialect))]
fn origin_reference_names_of<'py>(
    query_sql: String,
    recorded: &Bound<'py, PyAny>,
    renames: &Bound<'py, PyAny>,
    dialect: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let py: Python<'py> = recorded.py();
    let recorded_hash: String = native_field(recorded, "definition_hash")?.extract()?;
    let Some(mapping) = origin_names((query_sql, recorded), renames, (&recorded_hash, dialect))?
    else {
        return Ok(None);
    };
    let result: Bound<'py, PyDict> = PyDict::new(py);
    for (name, origin) in mapping {
        result.set_item(name, origin)?;
    }
    Ok(Some(result))
}

/// This plan's renames plus recorded renames, read only when a selected query that references models changed.
pub(crate) fn plan_reference_renames<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "plan_reference_renames",
        options,
        (
            &[
                "runtime", "scope", "snapshot", "state", "schemas", "planned",
            ],
            &[],
        ),
    )?;
    let (runtime, scope, snapshot) = (
        arguments.value("runtime")?,
        arguments.value("scope")?,
        arguments.value("snapshot")?,
    );
    let (state, schemas, planned) = (
        arguments.value("state")?,
        arguments.value("schemas")?,
        arguments.value("planned")?,
    );
    let (runtime, scope, snapshot) = (&runtime, &scope, &snapshot);
    if !changed_referencing_query(scope, snapshot)? {
        return Ok(PyTuple::empty(py));
    }
    let state_schemas: Bound<'py, PyAny> = native_field(snapshot, "migration_state_schemas")?;
    let inspected: Bound<'py, PySet> = PySet::empty(py)?;
    for schema in schemas.try_iter()? {
        let schema: Bound<'py, PyAny> = schema?;
        if state_schemas.contains(schema.call_method0("lower")?)? {
            inspected.add(schema)?;
        }
    }
    call_with(
        &native_field(&state, "inspect_migration_events")?,
        &[("schemas", inspected.into_any())],
    )?;
    let target_name: Bound<'py, PyAny> =
        native_field(&native_field(runtime, "project")?, "effective_target_name")?;
    let mut renames: Vec<Bound<'py, PyAny>> = Vec::new();
    for item in mapping_items(&planned)?.try_iter()? {
        let item: Bound<'py, PyAny> = item?;
        let (new_name, origin_name) = (item.get_item(0)?, item.get_item(1)?);
        if new_name.ne(&origin_name)? {
            renames.push(new_record_with(
                py,
                RecordId::ReferenceRename,
                &[("new_name", new_name), ("origin_name", origin_name)],
            )?);
        }
    }
    for event in native_field(&state, "events")?.try_iter()? {
        let event: Bound<'py, PyAny> = event?;
        let event_target: Bound<'py, PyAny> = native_field(&event, "target_name")?;
        let origin: Bound<'py, PyAny> = native_field(&event, "origin_model")?;
        let destination: Bound<'py, PyAny> = native_field(&event, "destination_model")?;
        if (event_target.is_none() || event_target.eq(&target_name)?)
            && !origin.is_none()
            && origin.ne(&destination)?
        {
            renames.push(new_record_with(
                py,
                RecordId::ReferenceRename,
                &[
                    ("new_name", destination),
                    ("origin_name", origin),
                    ("recorded_at", native_field(&event, "created_at")?),
                ],
            )?);
        }
    }
    PyTuple::new(py, renames)
}

fn changed_referencing_query(
    scope: &Bound<'_, PyAny>,
    snapshot: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let py: Python<'_> = scope.py();
    let scope = scope_data(scope)?;
    let mut selected: Vec<Bound<'_, PyAny>> = Vec::new();
    for (_, name) in &scope.selected {
        if let Some(model) = scope.model(py, name) {
            selected.push(model);
        }
    }
    let recorded_models: Bound<'_, PyAny> =
        native_field(&native_field(snapshot, "fingerprints")?, "models")?;
    let ref_kind: Bound<'_, PyAny> = enum_member(py, RecordId::SqlReferenceKind, "REF")?;
    let dialect: Option<String> = native_field(snapshot, "column_dialect")?.extract()?;
    for model in selected {
        let recorded: Bound<'_, PyAny> =
            mapping_get(&recorded_models, model_reads::name_object(&model)?)?;
        if recorded.is_none() {
            continue;
        }
        let mut references_models: bool = false;
        for reference in model_reads::references(&model)?.try_iter()? {
            if native_field(&reference?, "ref_kind")?.eq(&ref_kind)? {
                references_models = true;
                break;
            }
        }
        if references_models
            && model_query_hash(
                py,
                &model_reads::model_name(&model)?,
                (
                    &model_reads::query_sql(&model)?.extract::<String>()?,
                    dialect.as_deref(),
                ),
            )? != native_field(&recorded, "definition_hash")?.extract::<String>()?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(origin_reference_names_of, module)?)
}
