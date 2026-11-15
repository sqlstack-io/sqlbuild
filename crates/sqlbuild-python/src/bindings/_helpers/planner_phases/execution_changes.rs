//! Write-hash reconciliation and plan entry construction.

use crate::bindings::_helpers::command_runtime::concurrency::resolve_configured_concurrency;
use crate::bindings::_helpers::planner_entries::contexts::default_microbatch_limit_action;
use crate::bindings::_helpers::planner_entries::model_entries::plan_entries;
use crate::bindings::_helpers::planner_entries::models::{
    PlanEntriesRequest, PlanEntryBuildInputs, RelationsRequest,
};
use crate::bindings::_helpers::planner_entries::relations::planner_relations_context;
use crate::bindings::_helpers::planner_identity::identities::merge_recomputed_model_changes;
use crate::bindings::_helpers::planner_identity::identities::with_honest_model_write_hashes;
use crate::bindings::_helpers::planner_phases::contexts::class_field;
use crate::bindings::_helpers::planner_phases::models::PlannerIdentityContext;
use crate::bindings::_helpers::planner_phases::models::{ChangeReconciliation, ModelEntryResults};
use crate::bindings::_helpers::planner_phases::runtime::runtime_field;
use crate::bindings::_helpers::planner_phases::runtime::warehouse_field;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyFrozenSet;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The execution scope's changes with honest write hashes, and the actions merged from them.
pub(crate) fn reconcile_execution_changes<'py>(
    (execution_scope, snapshot): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    identities: &Bound<'py, PyAny>,
    resolved_actions: &Bound<'py, PyAny>,
    changes: &Bound<'py, PyAny>,
) -> PyResult<ChangeReconciliation<'py>> {
    let py = execution_scope.py();
    let honest = with_honest_model_write_hashes(
        py,
        Some(keywords(
            py,
            &[
                ("scope", execution_scope.clone()),
                ("snapshot", snapshot.clone()),
                ("changes", changes.clone()),
                (
                    "version_identities",
                    class_field(identities, |i: &PlannerIdentityContext| {
                        &i.version_identities
                    })?,
                ),
            ],
        )?),
    )?;
    let resolved = merge_recomputed_model_changes(resolved_actions, &honest)?;
    Ok(ChangeReconciliation {
        changes: honest,
        resolved_actions: resolved,
    })
}

fn config_or<'py>(
    config: &Bound<'py, PyAny>,
    path: &[&str],
    fallback: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if config.is_none() {
        return Ok(fallback);
    }
    let mut value = config.clone();
    for name in path {
        value = native_field(&value, name)?;
    }
    Ok(value)
}

/// The concurrency execution will use; without configs, the request or the compiled setting.
fn planned_concurrency(
    requested: &Bound<'_, PyAny>,
    (project_config, local_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    project: &Bound<'_, PyAny>,
) -> PyResult<i64> {
    if project_config.is_none() || local_config.is_none() {
        if !requested.is_none() {
            return requested.extract();
        }
        return native_field(&native_field(project, "settings")?, "concurrency")?.extract();
    }
    Ok(resolve_configured_concurrency(
        requested,
        (project_config, local_config),
        &native_field(project, "effective_target_name")?,
    )?
    .limit)
}

/// Model plan entries for the pruned execution scope, with the freshness and override inputs they need.
pub(crate) fn build_planner_entry_results<'py>(
    (runtime, warehouse): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (overrides, deferral): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (execution_scope, reconciliation): (&Bound<'py, PyAny>, &ChangeReconciliation<'py>),
    source_freshness: &Bound<'py, PyAny>,
) -> PyResult<ModelEntryResults<'py>> {
    let py = runtime.py();
    let project = runtime_field(runtime, |runtime| &runtime.project)?;
    let config = runtime_field(runtime, |runtime| &runtime.project_config)?;
    let relations = planner_relations_context(RelationsRequest {
        project: project.clone(),
        adapter: runtime_field(runtime, |runtime| &runtime.adapter)?,
        connection: runtime_field(runtime, |runtime| &runtime.connection)?,
        scope: execution_scope.clone(),
        deferral: deferral.clone(),
        project_config: config.clone(),
        local_config: runtime_field(runtime, |runtime| &runtime.local_config)?,
        known_source_columns: native_field(
            &native_field(warehouse, "inspection_relations")?,
            "source_warehouse_columns",
        )?,
        relation_keys: py.None().into_bound(py),
    })?;
    let propagation = native_field(source_freshness, "propagation")?;
    let blocked = if propagation.is_none() {
        PyFrozenSet::empty(py)?.into_any()
    } else {
        native_field(&propagation, "blocked_model_names")?
    };
    let none = || py.None().into_bound(py);
    let max_override = native_field(overrides, "max_microbatches")?;
    let max_microbatches = if max_override.is_none() {
        config_or(&config, &["microbatches", "limits", "max_batches"], none())?
    } else {
        max_override.clone()
    };
    let limit_action = if config.is_none() {
        default_microbatch_limit_action(py)?
    } else {
        native_field(
            &native_field(&native_field(&config, "microbatches")?, "limits")?,
            "action",
        )?
    };
    let build_inputs = Bound::new(
        py,
        PlanEntryBuildInputs {
            source_freshness_blocked_model_names: blocked.unbind(),
            external_blocked_model_names: PyFrozenSet::new(
                py,
                native_field(overrides, "external_blocked_model_names")?
                    .try_iter()?
                    .collect::<PyResult<Vec<_>>>()?,
            )?
            .into_any()
            .unbind(),
            start_cursor_override: none().unbind(),
            end_cursor_override: none().unbind(),
            future_cursor_config: config_or(&config, &["cursors", "future"], none())?.unbind(),
            start_cursor_config: config_or(&config, &["cursors", "start"], none())?.unbind(),
            invocation_time: runtime_field(runtime, |runtime| &runtime.invocation_time)?.unbind(),
            max_microbatches: max_microbatches.unbind(),
            max_microbatches_is_override: !max_override.is_none(),
            microbatch_limit_action: limit_action.unbind(),
        },
    )?
    .into_any();
    let concurrency: i64 = planned_concurrency(
        &native_field(overrides, "concurrency")?,
        (
            &config,
            &runtime_field(runtime, |runtime| &runtime.local_config)?,
        ),
        &project,
    )?;
    plan_entries(PlanEntriesRequest {
        project,
        adapter: runtime_field(runtime, |runtime| &runtime.adapter)?,
        scope: execution_scope.clone(),
        snapshot: warehouse_field(warehouse, |warehouse| &warehouse.snapshot)?,
        relations,
        resolved_models: native_field(&reconciliation.resolved_actions, "models")?,
        cursor_overrides: native_field(overrides, "cursor_overrides")?,
        full_refresh: native_field(overrides, "full_refresh")?.is_truthy()?,
        build_inputs,
        concurrency,
    })
}
