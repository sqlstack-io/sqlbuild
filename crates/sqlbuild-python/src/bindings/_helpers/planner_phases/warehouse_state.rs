//! The warehouse inspection phase: snapshot, migrations and inspection relations in one pass.

use crate::bindings::_helpers::graph::views::compiled_graph_keys;
use crate::bindings::_helpers::planner_entries::models::RelationsRequest;
use crate::bindings::_helpers::planner_entries::relations::planner_relations_context;
use crate::bindings::_helpers::planner_migrations::column_planning::plan_column_migrations;
use crate::bindings::_helpers::planner_migrations::model_planning::plan_model_migrations;
use crate::bindings::_helpers::planner_migrations::models::{
    ColumnMigrationInputs, MigrationPhase,
};
use crate::bindings::_helpers::planner_migrations::old_name_planning::plan_old_name_views as plan_old_name_migration_views;
use crate::bindings::_helpers::planner_phases::contexts::class_field;
use crate::bindings::_helpers::planner_phases::full_refresh::effectively_full_refreshed_model_names;
use crate::bindings::_helpers::planner_phases::models::PlannerScopeResolution;
use crate::bindings::_helpers::planner_phases::models::PlannerWarehouseState;
use crate::bindings::_helpers::planner_phases::objects::native;
use crate::bindings::_helpers::planner_phases::runtime::runtime_field;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::planner_warehouse::fingerprint_cache::MigrationFingerprintCache;
use crate::bindings::_helpers::planner_warehouse::snapshot::{SnapshotRequest, warehouse_snapshot};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use std::time::Instant;

fn progress(runtime: &Bound<'_, PyAny>, message: &str) -> PyResult<()> {
    let on_progress = runtime_field(runtime, |runtime| &runtime.on_progress)?;
    if !on_progress.is_none() {
        on_progress.call1((message,))?;
    }
    Ok(())
}

pub(crate) fn gather_planner_warehouse_state<'py>(
    runtime: &Bound<'py, PyAny>,
    scopes: &Bound<'py, PyAny>,
    overrides: &Bound<'py, PyAny>,
    deferral: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = runtime.py();
    let started = Instant::now();
    let project = runtime_field(runtime, |runtime| &runtime.project)?;
    let fingerprints = Py::new(
        py,
        MigrationFingerprintCache::open(py, Some(native_field(&project, "compile_cache_dir")?))?,
    )?
    .into_bound(py)
    .into_any();
    progress(runtime, "Inspecting warehouse state...")?;
    let selected_scope = class_field(scopes, |s: &PlannerScopeResolution| &s.selected_scope)?;
    let full_refresh: bool = native_field(overrides, "full_refresh")?.extract()?;
    let config = runtime_field(runtime, |runtime| &runtime.project_config)?;
    let cursor_scope = native(
        py,
        RecordId::CursorSnapshotScope,
        &[
            (
                "model_keys",
                native_field(&selected_scope, "selected_keys")?,
            ),
            (
                "runtime_producer_keys",
                native_field(&selected_scope, "selected_keys")?,
            ),
            (
                "invocation_time",
                runtime_field(runtime, |runtime| &runtime.invocation_time)?,
            ),
            (
                "start_cursor_config",
                if config.is_none() {
                    py.None().into_bound(py)
                } else {
                    native_field(&native_field(&config, "cursors")?, "start")?
                },
            ),
            (
                "cursor_overrides",
                native_field(overrides, "cursor_overrides")?,
            ),
        ],
    )?;
    let adapter = runtime_field(runtime, |runtime| &runtime.adapter)?;
    let stale_scope = scope_data(&class_field(scopes, |s: &PlannerScopeResolution| {
        &s.stale_warning_scope
    })?)?;
    let stale_keys = compiled_graph_keys(
        py,
        stale_scope
            .names
            .iter()
            .map(|(_, key)| key.clone())
            .collect(),
    )?
    .into_any();
    let snapshot = warehouse_snapshot(&SnapshotRequest {
        project: project.clone(),
        adapter: adapter.clone(),
        connection: runtime_field(runtime, |runtime| &runtime.connection)?,
        execute: native_field(&adapter, "execute")?,
        selected_keys: stale_keys,
        full_refresh: false,
        full_refresh_model_names: effectively_full_refreshed_model_names(&project, full_refresh)?,
        on_progress: runtime_field(runtime, |runtime| &runtime.on_progress)?,
        deferred_locations: native_field(deferral, "deferred_locations")?,
        cursor_scope,
    })?;
    let phase = MigrationPhase {
        runtime: runtime.clone(),
        scope: selected_scope.clone(),
        snapshot: snapshot.clone(),
        overrides: overrides.clone(),
        deferral: deferral.clone(),
    };
    let migrations = plan_model_migrations(&phase, &fingerprints)?;
    fingerprints.call_method0("persist")?;
    let inspection_relations = planner_relations_context(RelationsRequest {
        project: project.clone(),
        adapter: adapter.clone(),
        connection: runtime_field(runtime, |runtime| &runtime.connection)?,
        scope: class_field(scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?,
        deferral: deferral.clone(),
        project_config: config,
        local_config: runtime_field(runtime, |runtime| &runtime.local_config)?,
        known_source_columns: py.None().into_bound(py),
        relation_keys: py.None().into_bound(py),
    })?;
    let physical = PyDict::new(py);
    for entry in &migrations.entries {
        if native_field(&native_field(entry, "decision")?, "moves_data")?.is_truthy()? {
            let origin = native_field(entry, "origin")?;
            let qualified = native_field(&origin, "qualified_name")?;
            physical.set_item(
                native_field(entry, "model_name")?,
                if qualified.is_truthy()? {
                    qualified
                } else {
                    native_field(&origin, "name")?
                },
            )?;
        }
    }
    let columns = plan_column_migrations(
        &MigrationPhase {
            snapshot: migrations.snapshot.clone(),
            ..phase
        },
        &ColumnMigrationInputs {
            full_refresh_model_names: effectively_full_refreshed_model_names(
                &project,
                full_refresh,
            )?,
            physical_relations: physical.into_any(),
            source_columns: native_field(&inspection_relations, "source_warehouse_columns")?,
        },
    )?;
    let migration_entries = PyTuple::new(py, &migrations.entries)?.into_any();
    let column_entries = PyTuple::new(py, &columns.entries)?.into_any();
    let old_names = plan_old_name_migration_views(
        &MigrationPhase {
            runtime: runtime.clone(),
            scope: selected_scope,
            snapshot,
            overrides: overrides.clone(),
            deferral: deferral.clone(),
        },
        (&migration_entries, &column_entries),
    )?;
    if !runtime_field(runtime, |runtime| &runtime.on_progress)?.is_none() {
        progress(
            runtime,
            &format!(
                "Inspected warehouse state. ({:.2}s)",
                started.elapsed().as_secs_f64()
            ),
        )?;
        progress(runtime, "Generating plan...")?;
    }
    let mut warnings: Vec<_> = migrations.warnings;
    warnings.extend(columns.warnings);
    warnings.extend(old_names.warnings);
    Ok(Bound::new(
        py,
        PlannerWarehouseState {
            snapshot: columns.snapshot.unbind(),
            inspection_relations: inspection_relations.unbind(),
            migration_entries: migration_entries.unbind(),
            migration_warnings: PyTuple::new(py, warnings)?.into_any().unbind(),
            column_migration_entries: column_entries.unbind(),
            column_rename_hints: PyTuple::new(py, columns.hints)?.into_any().unbind(),
            old_name_view_entries: old_names.entries.unbind(),
            old_name_views: old_names.views.unbind(),
            migration_fingerprints: fingerprints.unbind(),
        },
    )?
    .into_any())
}
