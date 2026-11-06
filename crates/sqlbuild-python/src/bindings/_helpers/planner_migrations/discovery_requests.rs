//! Live candidate inspection and automatic request materialization.

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::graph::keys::native_key;
use crate::bindings::_helpers::planner_migrations::column_objects::config;
use crate::bindings::_helpers::planner_migrations::column_objects::native;
use crate::bindings::_helpers::planner_migrations::discovery_events::{
    identity, key, newest, relation,
};
use crate::bindings::_helpers::planner_migrations::models::{
    ModelMigrationDeclaration, ModelMigrationRequest,
};
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::construct::enum_member;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{
    PyDict, PyDictMethods, PyFrozenSet, PyListMethods, PySet, PySetMethods, PyString, PyTuple,
};
use sqlbuild_analysis::planner_migrations::main::discovery_materialization::discovery_materialization;
use sqlbuild_analysis::planner_migrations::main::stored_migration_fingerprint::stored_migration_fingerprint;
use std::collections::BTreeMap;

fn location<'py>(
    runtime: &Bound<'py, PyAny>,
    item: &Bound<'py, PyAny>,
    event: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let fields = if event {
        [
            ("database", native_field(item, "database")?),
            ("schema", native_field(item, "schema")?),
            ("name", native_field(item, "name")?),
        ]
    } else {
        let target = native_field(item, "target_name")?;
        let name = if target.is_truthy()? {
            target
        } else {
            native_field(item, "node_name")?
        };
        [
            ("database", native_field(item, "target_database")?),
            ("schema", native_field(item, "target_schema")?),
            ("name", name),
        ]
    };
    super::planning_origin::location(runtime, &fields)
}

pub(super) fn candidates<'py>(
    runtime: &Bound<'py, PyAny>,
    state: &Bound<'py, PyAny>,
    names: &Bound<'py, PyDict>,
    claims: (&[ModelMigrationDeclaration<'py>], &Bound<'py, PyDict>),
) -> PyResult<Bound<'py, PyTuple>> {
    let py = runtime.py();
    let excluded = PySet::empty(py)?;
    for declaration in claims.0 {
        excluded.add(key(&declaration.origin_location)?)?;
    }
    for (_, event) in claims.1.iter() {
        excluded.add(key(&native_field(&event, "origin")?)?)?;
    }
    let orphans = call_with(
        &native_field(state, "orphan_fingerprints")?,
        &[(
            "project_model_names",
            PyFrozenSet::new(py, names.keys().iter())?.into_any(),
        )],
    )?;
    let mut fingerprints: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut locations: Vec<Bound<'py, PyAny>> = Vec::new();
    for item in orphans.try_iter()? {
        let item = item?;
        let metadata: String = native_field(&item, "metadata_json")?.extract()?;
        if stored_migration_fingerprint(&metadata).is_some()
            && !native_field(&item, "target_name")?.is_none()
        {
            locations.push(location(runtime, &item, false)?);
            fingerprints.push(item);
        }
    }
    call_with(
        &native_field(state, "inspect_relations")?,
        &[("locations", PyTuple::new(py, &locations)?.into_any())],
    )?;
    let mut result: Vec<Bound<'py, PyAny>> = Vec::new();
    for (fingerprint, location) in fingerprints.into_iter().zip(locations) {
        if state.call_method1("relation", (&location,))?.is_none()
            || excluded.contains(key(&location)?)?
        {
            continue;
        }
        let event = newest(&native_field(state, "events")?, &location)?;
        if !event.is_none()
            && native_field(&event, "origin")?
                .call_method1("matches", (relation(&location)?,))?
                .is_truthy()?
        {
            continue;
        }
        result.push(fingerprint);
    }
    PyTuple::new(py, result)
}

pub(super) fn automatic<'py>(
    runtime: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
    models: &[Bound<'py, PyAny>],
    matches: (&Bound<'py, PyAny>, &Bound<'py, PyDict>),
) -> PyResult<Vec<ModelMigrationRequest<'py>>> {
    let py = runtime.py();
    let scope = scope_data(scope)?;
    let mut requests: Vec<ModelMigrationRequest<'py>> = Vec::new();
    for item in models {
        if !scope
            .selected
            .contains(&native_key(&model_reads::model_key(item)?)?)
        {
            continue;
        }
        let name = model_reads::name_object(item)?;
        let event = matches.1.get_item(&name)?;
        let candidate = mapping_get(matches.0, &name)?;
        let origin = if let Some(event) = event {
            Some((
                location(runtime, &native_field(&event, "origin")?, true)?,
                native_field(&event, "origin_model")?,
            ))
        } else if !candidate.is_none() {
            let materialized: Option<String> = config(item, "materialized")?.extract()?;
            let metadata: String = native_field(&candidate, "metadata_json")?.extract()?;
            if discovery_materialization(materialized.as_deref().unwrap_or_default(), &metadata) {
                Some((
                    location(runtime, &candidate, false)?,
                    native_field(&candidate, "node_name")?,
                ))
            } else {
                None
            }
        } else {
            None
        };
        if let Some((location, origin_model)) = origin {
            requests.push(ModelMigrationRequest {
                model: item.clone(),
                discovery: enum_member(py, RecordId::MigrationDiscovery, "AUTOMATIC")?,
                raw_origin: None,
                origin_location: Some(location),
                origin_model,
                force: false,
                identity_only: identity(item)?,
            });
        }
    }
    Ok(requests)
}

pub(super) fn warnings<'py>(
    models: &[Bound<'py, PyAny>],
    scope: &Bound<'py, PyAny>,
    ambiguous: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = scope.py();
    let scope = scope_data(scope)?;
    let selected = PySet::empty(py)?;
    for item in models {
        if scope
            .selected
            .contains(&native_key(&model_reads::model_key(item)?)?)
        {
            selected.add(model_reads::name_object(item)?)?;
        }
    }
    let ambiguous: BTreeMap<String, String> = ambiguous.extract()?;
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    for (name, detail) in ambiguous {
        if selected.contains(&name)? {
            warnings.push(native(py, RecordId::PlanWarning, &[
                ("model_name", PyString::new(py, &name).into_any()),
                ("severity", enum_member(py, RecordId::WarningSeverity, "WARNING")?),
                ("message", PyString::new(py, &format!("'{name}' matches {detail}; no automatic migration was inferred. Add migrate_from to choose one.")).into_any()),
                ("code", PyString::new(py, "M107").into_any()),
            ])?);
        }
    }
    PyTuple::new(py, warnings)
}
