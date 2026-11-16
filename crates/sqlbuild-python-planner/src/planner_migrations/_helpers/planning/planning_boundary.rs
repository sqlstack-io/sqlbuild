//! Assemble migration policy and redirected live cursor inspection requests.

use crate::planner_migrations::_helpers::column::column_objects::native;
use crate::planner_migrations::models::MigrationPhase;
use crate::planner_phases::main::inputs::runtime_field::runtime_field;
use crate::planner_phases::main::rebuilds::effective_target_config::effective_target_config;
use crate::planner_phases::main::rebuilds::effectively_full_refreshed_model_names::effectively_full_refreshed_model_names;
use crate::planner_warehouse::main::inspection::gather_redirected_cursor_snapshots::gather_redirected_cursor_snapshots;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// The target's policy for migration origins that no longer exist.
pub(crate) fn missing_origin_policy<'py>(
    runtime: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    native_field(
        &effective_target_config(runtime)?,
        "missing_migration_origin",
    )
}

/// Live cursor snapshots of models whose history now sits in a redirected relation.
pub(crate) fn redirected_cursor_snapshots<'py>(
    phase: &MigrationPhase<'py>,
    (existing_relations, target_relations): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    origin_cursor_columns: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let runtime = &phase.runtime;
    let py = runtime.py();
    let overrides = &phase.overrides;
    let scope = &phase.scope;
    let project = runtime_field(runtime, |runtime| &runtime.project)?;
    let config = runtime_field(runtime, |runtime| &runtime.project_config)?;
    let start = if config.is_none() {
        py.None().into_bound(py)
    } else {
        native_field(&native_field(&config, "cursors")?, "start")?
    };
    let cursor_scope = native(
        py,
        RecordId::CursorSnapshotScope,
        &[
            ("model_keys", native_field(scope, "selected_keys")?),
            (
                "runtime_producer_keys",
                native_field(scope, "selected_keys")?,
            ),
            (
                "invocation_time",
                runtime_field(runtime, |runtime| &runtime.invocation_time)?,
            ),
            ("start_cursor_config", start),
            (
                "cursor_overrides",
                native_field(overrides, "cursor_overrides")?,
            ),
        ],
    )?;
    let full_refresh = effectively_full_refreshed_model_names(
        &project,
        native_field(overrides, "full_refresh")?.is_truthy()?,
    )?;
    gather_redirected_cursor_snapshots(
        py,
        Some(keywords(
            py,
            &[
                ("project", project),
                (
                    "adapter",
                    runtime_field(runtime, |runtime| &runtime.adapter)?,
                ),
                (
                    "connection",
                    runtime_field(runtime, |runtime| &runtime.connection)?,
                ),
                ("existing_relations", existing_relations),
                ("target_relations", target_relations),
                ("cursor_scope", cursor_scope),
                ("full_refresh_model_names", full_refresh),
                (
                    "deferred_locations",
                    native_field(&phase.deferral, "deferred_locations")?,
                ),
                (
                    "on_progress",
                    runtime_field(runtime, |runtime| &runtime.on_progress)?,
                ),
                (
                    "origin_cursor_columns",
                    origin_cursor_columns.unwrap_or_else(|| py.None().into_bound(py)),
                ),
            ],
        )?),
    )
}
