//! Model migration orchestration over Python-owned live inspection.

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::model_config::typed_config::{
    config_node_of, config_value, model_values,
};
use crate::bindings::_helpers::planner_changes::renames::plan_reference_renames;
use crate::bindings::_helpers::planner_migrations::column_objects::{config, replace};
use crate::bindings::_helpers::planner_migrations::discovery::discover_model_migrations;
use crate::bindings::_helpers::planner_migrations::models::{
    DiscoveryRequest, MigrationDecisionRequest, MigrationPhase, ModelMigrationDeclaration,
    ModelMigrationDiscovery, ModelMigrationPlanning, ModelMigrationRequest,
};
use crate::bindings::_helpers::planner_migrations::planning_origin::{
    require_same_database, resolve_origin,
};
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::planner_phases::runtime::runtime_field;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::construct::{enum_member, new_record};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get_or;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PySet, PySetMethods, PyString, PyTuple};
use sqlbuild_analysis::planner_identity::constants::MODEL_RESOURCE;
use sqlbuild_model_config::config_values::models::ConfigNode;

const MIGRATE_FROM_KEY: &str = "migrate_from";
const MIGRATE_FORCE_KEY: &str = "migrate_force";

fn declared<'py>(item: &Bound<'py, PyAny>) -> PyResult<Option<ModelMigrationRequest<'py>>> {
    let py = item.py();
    let values = model_values(item)?;
    let origin = config_value(py, &values, MIGRATE_FROM_KEY)?;
    if !origin.is_instance_of::<PyString>() {
        return Ok(None);
    }
    let materialization = config(item, "materialized")?;
    Ok(Some(ModelMigrationRequest {
        model: item.clone(),
        discovery: enum_member(py, RecordId::MigrationDiscovery, "MANUAL")?,
        raw_origin: Some(origin.call_method0("strip")?),
        origin_location: None,
        origin_model: py.None().into_bound(py),
        force: matches!(
            *config_node_of(&values, MIGRATE_FORCE_KEY)?,
            ConfigNode::Bool(true)
        ),
        identity_only: materialization.eq("table")? || materialization.eq("view")?,
    }))
}

fn manual_migration_requests<'py>(
    scope: &Bound<'py, PyAny>,
) -> PyResult<Vec<ModelMigrationRequest<'py>>> {
    let py = scope.py();
    let scope = scope_data(scope)?;
    let mut requests: Vec<ModelMigrationRequest<'py>> = Vec::new();
    for key in &scope.order {
        if !scope.selected.contains(key) || key.0 != MODEL_RESOURCE {
            continue;
        }
        let Some(item) = scope.model(py, &key.1) else {
            continue;
        };
        if !item.is_none()
            && let Some(request) = declared(&item)?
        {
            requests.push(request);
        }
    }
    Ok(requests)
}

/// Model migrations of the selected scope: discovered and declared moves, decided against live state.
pub(crate) fn plan_model_migrations<'py>(
    phase: &MigrationPhase<'py>,
    fingerprints: &Bound<'py, PyAny>,
) -> PyResult<ModelMigrationPlanning<'py>> {
    let runtime = phase.runtime.clone();
    let snapshot = phase.snapshot.clone();
    let schemas = super::planning_origin::project_schemas(&runtime)?;
    let state = super::state_inspection::inspection(
        &runtime,
        Some(&native_field(&snapshot, "fingerprints")?),
    )?;
    let discovery = discover((phase, fingerprints), &state, &schemas)?;
    finish(phase, &state, &schemas, &discovery)
}

fn discover<'py>(
    (phase, fingerprints): (&MigrationPhase<'py>, &Bound<'py, PyAny>),
    state: &Bound<'py, PyAny>,
    schemas: &Bound<'py, PyAny>,
) -> PyResult<ModelMigrationDiscovery<'py>> {
    let runtime = phase.runtime.clone();
    let scope = phase.scope.clone();
    let snapshot = phase.snapshot.clone();
    let mut declared_requests: Vec<ModelMigrationRequest<'py>> = Vec::new();
    for item in
        project_reads::models(&runtime_field(&runtime, |runtime| &runtime.project)?)?.try_iter()?
    {
        if let Some(request) = declared(&item?)? {
            declared_requests.push(request);
        }
    }
    if !declared_requests.is_empty() {
        call_with(
            &native_field(state, "inspect_schemas")?,
            &[("schemas", schemas.clone())],
        )?;
    }
    let mut declarations: Vec<ModelMigrationDeclaration<'py>> = Vec::new();
    for request in &declared_requests {
        let (origin_location, origin_model) = resolve_origin(request, &runtime, state)?;
        declarations.push(ModelMigrationDeclaration {
            model_name: model_reads::name_object(&request.model)?,
            origin_location,
            origin_model,
        });
    }
    discover_model_migrations(DiscoveryRequest {
        manual_requests: manual_migration_requests(&scope)?,
        runtime,
        scope,
        snapshot,
        declarations,
        state: state.clone(),
        project_schemas: schemas.clone(),
        fingerprints: fingerprints.clone(),
    })
}

fn finish<'py>(
    phase: &MigrationPhase<'py>,
    state: &Bound<'py, PyAny>,
    schemas: &Bound<'py, PyAny>,
    discovery: &ModelMigrationDiscovery<'py>,
) -> PyResult<ModelMigrationPlanning<'py>> {
    let runtime = phase.runtime.clone();
    let py = runtime.py();
    let scope = phase.scope.clone();
    let snapshot = phase.snapshot.clone();
    let mut resolved: Vec<(
        &ModelMigrationRequest<'py>,
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
    )> = Vec::new();
    for request in &discovery.requests {
        let (origin, origin_model) = resolve_origin(request, &runtime, state)?;
        resolved.push((request, origin, origin_model));
    }
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    let handovers = PyDict::new(py);
    if !resolved.is_empty() {
        let origin_schemas = PySet::empty(py)?;
        let mut origins: Vec<Bound<'py, PyAny>> = Vec::new();
        for (_, origin, _) in &resolved {
            require_same_database(state, origin)?;
            let schema = native_field(origin, "schema")?;
            if !schema.is_none() {
                origin_schemas.add(schema)?;
            }
            origins.push(origin.clone());
        }
        call_with(
            &native_field(state, "inspect_schemas")?,
            &[("schemas", origin_schemas.into_any())],
        )?;
        call_with(
            &native_field(state, "inspect_relations")?,
            &[("locations", PyTuple::new(py, origins)?.into_any())],
        )?;
        let project_config = runtime_field(&runtime, |runtime| &runtime.project_config)?;
        let snapshots_config = if project_config.is_none() {
            new_record(py, RecordId::SnapshotsConfig, (), None)?
        } else {
            native_field(&project_config, "snapshots")?
        };
        let policy = super::planning_boundary::missing_origin_policy(&runtime)?;
        for (request, origin, origin_model) in resolved {
            let item = request.model.clone();
            let name = model_reads::name_object(&item)?;
            let decided = super::model_entries::decide(&MigrationDecisionRequest {
                runtime: runtime.clone(),
                request,
                origin,
                origin_model,
                snapshot: snapshot.clone(),
                state: state.clone(),
                snapshots_config: snapshots_config.clone(),
            })?;
            entries.push(replace(
                &decided.entry,
                &[("missing_origin_policy", policy.clone())],
            )?);
            if decided.applies {
                let destinations = mapping_get_or(
                    &discovery.destination_fingerprints,
                    &name,
                    PyTuple::empty(py),
                )?;
                handovers.set_item(
                    name,
                    super::model_handover::equivalent(
                        &item,
                        decided.inherited,
                        &destinations,
                        &native_field(&snapshot, "column_dialect")?,
                    )?,
                )?;
            }
        }
    }
    let planned = PyDict::new(py);
    let mut renamed: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut warnings: Vec<Bound<'py, PyAny>> = Vec::new();
    warnings.extend(discovery.warnings.iter().cloned());
    for entry in &entries {
        let name = native_field(entry, "model_name")?;
        if let Some(handover) = handovers.get_item(&name)? {
            if !handover.is_none() && native_field(entry, "decision")?.eq("renamed")? {
                renamed.push(name.clone());
            }
            let origin_model = native_field(entry, "origin_model")?;
            if !origin_model.is_none() {
                planned.set_item(&name, origin_model)?;
            }
        }
        if let Some(warning) = super::model_warnings::entry_warning(entry)? {
            warnings.push(warning);
        }
    }
    let overlaid = if entries.is_empty() {
        snapshot.clone()
    } else {
        super::model_overlay::overlay(phase, state, &entries, &handovers)?
    };
    let reference_renames = object(
        py,
        plan_reference_renames(
            py,
            Some(keywords(
                py,
                &[
                    ("runtime", runtime),
                    ("scope", scope),
                    ("snapshot", snapshot),
                    ("state", state.clone()),
                    ("schemas", schemas.clone()),
                    ("planned", planned.into_any()),
                ],
            )?),
        )?,
    )?;
    let mut fields = vec![("reference_renames", reference_renames)];
    if !entries.is_empty() {
        fields.push(("renamed_models", PyFrozenSet::new(py, renamed)?.into_any()));
    }
    Ok(ModelMigrationPlanning {
        snapshot: replace(&overlaid, &fields)?,
        entries,
        warnings,
    })
}
