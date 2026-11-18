//! Write-hash reconciliation and plan entry construction.

use crate::planner_entries::main::entries::default_microbatch_limit_action::default_microbatch_limit_action;
use crate::planner_entries::main::entries::plan_entries::plan_entries;
use crate::planner_entries::models::PlanEntriesRequest;
use crate::planner_entries::models::PlanEntryBuildInputs;
use crate::planner_phases::_helpers::inputs::runtime::runtime_field;
use crate::planner_phases::_helpers::inputs::runtime::warehouse_field;
use crate::planner_phases::main::inputs::runtime_dialect::runtime_dialect;
use crate::planner_phases::models::ChangeReconciliation;
use crate::planner_phases::models::ModelEntryResults;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyFrozenSet;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::command_runtime::main::resolve_configured_concurrency::resolve_configured_concurrency;

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

/// Model plan entries for the execution scope over the relations its warehouse phase resolved.
pub(crate) fn build_planner_entry_results<'py>(
    (runtime, warehouse): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    overrides: &Bound<'py, PyAny>,
    (execution_scope, reconciliation): (&Bound<'py, PyAny>, &ChangeReconciliation<'py>),
    source_freshness: &Bound<'py, PyAny>,
) -> PyResult<ModelEntryResults<'py>> {
    let py = runtime.py();
    let project = runtime_field(runtime, |runtime| &runtime.project)?;
    let config = runtime_field(runtime, |runtime| &runtime.project_config)?;
    let relations = native_field(warehouse, "inspection_relations")?;
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
        dialect: runtime_dialect(runtime)?,
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
