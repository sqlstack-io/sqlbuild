//! Compatibility views at old model names and pending migration origins the janitor protects.

use std::collections::{HashMap, HashSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::janitor_planning::main::migration_origin_keys::migration_origin_keys;
use sqlbuild_analysis::janitor_planning::main::old_name_request_matches::old_name_request_matches;

use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::janitor::janitor_facts::{
    SchemaRelations, Warehouse, model_destinations,
};
use crate::bindings::_helpers::janitor::janitor_keys::{
    Folded, Key, SchemaKey, optional, record, sorted_schemas, text, tuple,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_front::state_rows::main::migrations::read_histories::read_histories as read_view_histories;
use sqlbuild_python_front::state_rows::main::migrations::stored_columns_binding::stored_columns_binding;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;

const IS_COMPATIBILITY_VIEW: ExecutorHost = ExecutorHost::IsCompatibilityView;
use sqlbuild_core::constants::MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::OLD_NAME_VIEW_TABLE_NAME;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
const MIGRATE_FROM_CONFIG_KEY: &str = "migrate_from";

/// Old-name view planning and the old names it owns.
pub(crate) struct OldNamePlanning<'py> {
    pub(crate) planning: Bound<'py, PyAny>,
    pub(crate) keys: HashSet<Folded>,
}

fn drop_reason<'py>(py: Python<'py>, member: &str) -> PyResult<Bound<'py, PyAny>> {
    enum_member(py, RecordId::OldNameViewDropReason, member)
}

fn status<'py>(
    history: &Bound<'py, PyAny>,
    now: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(&native_field(history, "status")?, &[("now", now.clone())])
}

fn status_is(py: Python<'_>, value: &Bound<'_, PyAny>, member: &str) -> PyResult<bool> {
    value.eq(enum_member(py, RecordId::OldNameViewStatus, member)?)
}

fn read_histories<'py>(
    warehouse: &Warehouse<'py>,
    managed: &[SchemaKey],
    relations: &SchemaRelations<'py>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = warehouse.py();
    let mut histories: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in sorted_schemas(managed) {
        let names: HashSet<String> = match relations.iter().find(|(listed, _)| *listed == key) {
            Some((_, listed)) => listed
                .iter()
                .map(|relation| {
                    Ok(native_field(relation, "name")?
                        .extract::<String>()?
                        .to_lowercase())
                })
                .collect::<PyResult<_>>()?,
            None => HashSet::new(),
        };
        let (database, Some(schema)) = key else {
            continue;
        };
        if !names.contains(OLD_NAME_VIEW_TABLE_NAME) || !names.contains(MIGRATION_TABLE_NAME) {
            continue;
        }
        let stored_columns: Bound<'py, PyAny> = stored_columns_binding(
            &warehouse.adapter,
            &warehouse.connection,
            optional(py, database.as_deref())?,
            text(py, &schema),
        )?
        .into_any();
        let read = read_view_histories(Some(&keywords(
            py,
            &[
                ("connection", warehouse.connection.clone()),
                ("execute", native_field(&warehouse.adapter, "execute")?),
                ("database", optional(py, database.as_deref())?),
                ("schema", text(py, &schema)),
                ("stored_columns", stored_columns),
                (
                    "render_qualified_name",
                    native_field(&warehouse.adapter, "render_qualified_name")?,
                ),
            ],
        )?))?;
        for history in read.try_iter()? {
            histories.push(history?);
        }
    }
    Ok(histories)
}

fn old_key(history: &Bound<'_, PyAny>) -> PyResult<Key> {
    Key::of(&native_field(history, "old")?)
}

/// Each history's listed relation at its old name, listing only schemas not already scanned.
fn physical_old_names<'py>(
    warehouse: &Warehouse<'py>,
    histories: &[Bound<'py, PyAny>],
    relations: &SchemaRelations<'py>,
) -> PyResult<HashMap<Folded, Bound<'py, PyAny>>> {
    let mut known: Vec<(String, &Vec<Bound<'py, PyAny>>)> = Vec::new();
    for ((_, schema), listed) in relations {
        let folded: String = schema.as_deref().unwrap_or("").to_lowercase();
        match known.iter_mut().find(|(name, _)| *name == folded) {
            Some(entry) => entry.1 = listed,
            None => known.push((folded, listed)),
        }
    }
    let mut unscanned: Vec<Key> = Vec::new();
    for history in histories {
        let key: Key = old_key(history)?;
        let folded: String = key.schema.as_deref().unwrap_or("").to_lowercase();
        if !known.iter().any(|(name, _)| *name == folded) {
            unscanned.push(key);
        }
    }
    let mut scanned: Vec<Bound<'py, PyAny>> = Vec::new();
    if let Some(first) = unscanned.first() {
        let mut schemas: Vec<String> = unscanned
            .iter()
            .map(|key| key.schema.clone().unwrap_or_default())
            .collect();
        schemas.sort();
        schemas.dedup();
        let mut names: Vec<String> = unscanned.iter().map(|key| key.name.clone()).collect();
        names.sort();
        names.dedup();
        let listed: Bound<'py, PyAny> =
            warehouse.list_relations(first.database.as_deref(), (Some(&schemas), Some(&names)))?;
        for relation in listed.try_iter()? {
            scanned.push(relation?);
        }
    }
    for (_, listed) in &known {
        scanned.extend(listed.iter().cloned());
    }
    let mut found: Vec<(Key, Bound<'py, PyAny>)> = Vec::new();
    let mut seen: HashSet<Folded> = HashSet::new();
    for relation in scanned {
        let key: Key = Key::of(&relation)?;
        if seen.insert(key.folded()) {
            found.push((key, relation));
        }
    }
    let mut physical: HashMap<Folded, Bound<'py, PyAny>> = HashMap::new();
    for history in histories {
        let key: Key = old_key(history)?;
        let exact: Option<&Bound<'py, PyAny>> = found
            .iter()
            .find(|(candidate, _)| candidate.folded() == key.folded())
            .map(|(_, relation)| relation);
        let matched: Option<&Bound<'py, PyAny>> = if exact.is_some() || key.database.is_some() {
            exact
        } else {
            found
                .iter()
                .find(|(candidate, _)| {
                    candidate.schema.as_deref().unwrap_or("").to_lowercase()
                        == key.schema.as_deref().unwrap_or("").to_lowercase()
                        && candidate.name.to_lowercase() == key.name.to_lowercase()
                })
                .map(|(_, relation)| relation)
        };
        if let Some(relation) = matched {
            physical.insert(key.folded(), relation.clone());
        }
    }
    Ok(physical)
}

fn view<'py>(
    (key, history): (&Key, &Bound<'py, PyAny>),
    reason: Option<&str>,
    claimed_by: Option<Option<String>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = history.py();
    let created: Bound<'py, PyAny> = native_field(history, "created")?;
    let expires_at: Bound<'py, PyAny> = if created.is_none() {
        py.None().into_bound(py)
    } else {
        native_field(&created, "expires_at")?
    };
    let mut pairs: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("key", key.to_python(py)?),
        ("history", history.clone()),
        ("expires_at", expires_at),
    ];
    if let Some(reason) = reason {
        pairs.push(("drop_reason", drop_reason(py, reason)?));
    }
    if let Some(claimed_by) = claimed_by {
        pairs.push((
            "occupied",
            pyo3::types::PyBool::new(py, true).to_owned().into_any(),
        ));
        pairs.push(("claimed_by", optional(py, claimed_by.as_deref())?));
    }
    record(py, ExecutorHost::JanitorOldNameView, &pairs)
}

/// Each project model's destination mapped to the model's name.
fn project_destinations<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = project.py();
    let destinations: Bound<'py, PyDict> = PyDict::new(py);
    for (key, model) in model_destinations(project)? {
        destinations.set_item(key.to_python(py)?, name_object(&model)?)?;
    }
    Ok(destinations)
}

fn claiming_model(destinations: &Bound<'_, PyDict>, key: &Key) -> PyResult<Option<String>> {
    let lower = |value: Option<&str>| value.unwrap_or("").to_lowercase();
    for (destination, model_name) in destinations.iter() {
        let destination: Key = Key::of(&destination)?;
        if lower(destination.schema.as_deref()) == lower(key.schema.as_deref())
            && destination.name.to_lowercase() == key.name.to_lowercase()
            && (destination.database.is_none()
                || key.database.is_none()
                || lower(destination.database.as_deref()) == lower(key.database.as_deref()))
        {
            return model_name.extract().map(Some);
        }
    }
    Ok(None)
}

/// Recorded views split into live, droppable and gone; another relation at the name is gone.
pub(crate) fn plan_old_name_views<'py>(
    (project, warehouse): (&Bound<'py, PyAny>, &Warehouse<'py>),
    (managed, relations): (&[SchemaKey], &SchemaRelations<'py>),
    (early_drops, now): (&[String], &Bound<'py, PyAny>),
) -> PyResult<OldNamePlanning<'py>> {
    let py: Python<'py> = warehouse.py();
    let target_name: Option<String> = native_field(project, "effective_target_name")?.extract()?;
    let destinations: Bound<'py, PyDict> = project_destinations(project)?;
    let mut histories: Vec<Bound<'py, PyAny>> = Vec::new();
    for history in read_histories(warehouse, managed, relations)? {
        let recorded: Option<String> = native_field(&history, "move")?
            .getattr("target_name")?
            .extract()?;
        if recorded.is_some() && recorded != target_name {
            continue;
        }
        let current: Bound<'py, PyAny> = status(&history, now)?;
        if status_is(py, &current, "LIVE")? || status_is(py, &current, "EXPIRED")? {
            histories.push(history);
        }
    }
    if histories.is_empty() && early_drops.is_empty() {
        return Ok(OldNamePlanning {
            planning: record(py, ExecutorHost::JanitorOldNameViewPlanning, &[])?,
            keys: HashSet::new(),
        });
    }
    let physical: HashMap<Folded, Bound<'py, PyAny>> =
        physical_old_names(warehouse, &histories, relations)?;
    let mut requested: Vec<(String, bool)> = Vec::new();
    for name in early_drops {
        if !requested.iter().any(|(listed, _)| listed == name) {
            requested.push((name.clone(), false));
        }
    }
    let mut definitions: Vec<(Option<String>, String, Vec<String>)> = Vec::new();
    for history in &histories {
        let key: Key = old_key(history)?;
        let (Some(schema), true) = (&key.schema, physical.contains_key(&key.folded())) else {
            continue;
        };
        match definitions
            .iter_mut()
            .find(|(database, listed, _)| *database == key.database && listed == schema)
        {
            Some((_, _, names)) => names.push(key.name.clone()),
            None => {
                definitions.push((key.database.clone(), schema.clone(), vec![key.name.clone()]))
            }
        }
    }
    warehouse.prefetch_view_definitions(&definitions)?;
    let is_compatibility_view: Bound<'py, PyAny> = host_entry(py, IS_COMPATIBILITY_VIEW)?;
    let (mut live, mut drops, mut missing) = (Vec::new(), Vec::new(), Vec::new());
    let mut keys: HashSet<Folded> = HashSet::new();
    for history in &histories {
        let key: Key = old_key(history)?;
        keys.insert(key.folded());
        let plain: Bound<'py, PyAny> = view((&key, history), None, None)?;
        let mut matched: bool = false;
        for (name, found) in &mut requested {
            if old_name_request_matches(
                name,
                (key.database.as_deref(), key.schema.as_deref(), &key.name),
            ) {
                *found = true;
                matched = true;
            }
        }
        let Some(relation) = physical.get(&key.folded()) else {
            missing.push(view((&key, history), Some("MISSING"), None)?);
            continue;
        };
        let compatible: bool = call_with(
            &is_compatibility_view,
            &[
                ("adapter", warehouse.adapter.clone()),
                ("connection", warehouse.connection.clone()),
                ("view", plain.clone()),
                ("relation", relation.clone()),
            ],
        )?
        .is_truthy()?;
        if !compatible {
            let claimed_by: Option<String> = claiming_model(&destinations, &key)?;
            missing.push(view((&key, history), Some("MISSING"), Some(claimed_by))?);
        } else if matched {
            drops.push(view((&key, history), Some("EARLY"), None)?);
        } else if status_is(py, &status(history, now)?, "EXPIRED")? {
            drops.push(view((&key, history), Some("EXPIRED"), None)?);
        } else {
            live.push(plain);
        }
    }
    let unknown: Vec<Bound<'py, PyAny>> = requested
        .iter()
        .filter(|(_, found)| !found)
        .map(|(name, _)| text(py, name))
        .collect();
    let planning: Bound<'py, PyAny> = record(
        py,
        ExecutorHost::JanitorOldNameViewPlanning,
        &[
            ("live", tuple(py, live)?),
            ("drops", tuple(py, drops)?),
            ("missing", tuple(py, missing)?),
            ("unknown_requests", tuple(py, unknown)?),
            ("project_destinations", destinations.into_any()),
        ],
    )?;
    Ok(OldNamePlanning { planning, keys })
}

/// Listed origins of migrations whose destination relation does not exist yet, with reasons.
pub(crate) fn pending_migration_origins(
    project: &Bound<'_, PyAny>,
    (relations, managed): (&SchemaRelations<'_>, &[SchemaKey]),
) -> PyResult<Vec<(Folded, String)>> {
    let mut listed: HashSet<Folded> = HashSet::new();
    for (_, schema_relations) in relations {
        for relation in schema_relations {
            listed.insert(Key::of(relation)?.folded());
        }
    }
    let schemas: Vec<SchemaKey> = sorted_schemas(managed);
    let mut pending: Vec<(Folded, String)> = Vec::new();
    for (destination, model) in model_destinations(project)? {
        let raw: Bound<'_, PyAny> = native_field(&model, "config")?
            .getattr("values")?
            .call_method1("get", (MIGRATE_FROM_CONFIG_KEY,))?;
        let Ok(raw) = raw.extract::<String>() else {
            continue;
        };
        if listed.contains(&destination.folded()) {
            continue;
        }
        let model_name: String =
            sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name(
                &model,
            )?;
        for origin in migration_origin_keys(&raw, destination.database.as_deref(), &schemas) {
            let folded: Folded = Key {
                database: origin.database,
                schema: origin.schema,
                name: origin.name,
            }
            .folded();
            if listed.contains(&folded) && !pending.iter().any(|(key, _)| *key == folded) {
                pending.push((folded, format!("pending migration origin for {model_name}")));
            }
        }
    }
    Ok(pending)
}
