//! Top-level planner orchestration producing an execution plan.

use sqlbuild_analysis::planner_scope::constants::MODEL;
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, within};
use crate::bindings::_helpers::build_command::support::utc_now;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_changes::actions::resolve_model_actions;
use crate::bindings::_helpers::planner_changes::detect::detect_changes;
use crate::bindings::_helpers::planner_entries::test_planning::{
    PlanningRequest, SqlTestPlanning, start_planning,
};
use crate::bindings::_helpers::planner_entries::values::{native, replaced};
use crate::bindings::_helpers::planner_phases::buildability::check_selected_scope_buildability;
use crate::bindings::_helpers::planner_phases::contexts::class_field;
use crate::bindings::_helpers::planner_phases::deferred_origins::defer_to_migration_origins;
use crate::bindings::_helpers::planner_phases::execution_changes::{
    build_planner_entry_results, reconcile_execution_changes,
};
use crate::bindings::_helpers::planner_phases::identities::{
    build_planner_identity_context, detect_stale_warning_changes,
};
use crate::bindings::_helpers::planner_phases::models::PlannerIdentityContext;
use crate::bindings::_helpers::planner_phases::models::PlannerRuntime;
use crate::bindings::_helpers::planner_phases::models::PlannerScopeResolution;
use crate::bindings::_helpers::planner_phases::models::{
    PlanOutputContext, PlanWarningContext, PlannedModelEntries, StoragePolicyContext,
};
use crate::bindings::_helpers::planner_phases::output_assembly::unselected_upstream_models;
use crate::bindings::_helpers::planner_phases::output_assembly::{
    add_deferred_origin_notes, add_plan_metadata, add_plan_warnings, add_storage_policies,
    base_plan_output,
};
use crate::bindings::_helpers::planner_phases::runtime::warehouse_field;
use crate::bindings::_helpers::planner_phases::scopes::resolve_planner_scopes;
use crate::bindings::_helpers::planner_phases::warehouse_state::gather_planner_warehouse_state;
use crate::bindings::_helpers::planner_resolution::cursor_overrides::type_cursor_overrides;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::planner_warehouse::source_freshness::build_planner_source_freshness_result;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use std::time::Instant;

const INSPECTION_CATALOG: AdapterHost = AdapterHost::OpenInspectionCatalog;
/// Selected scopes, and the overrides with cursor overrides typed for the selected models.
fn scopes_and_cursor_overrides<'py>(
    args: &KeywordArguments<'py>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let project = args.value("project")?;
    let py = project.py();
    let scopes = resolve_planner_scopes(
        &project,
        &args.value("selection")?,
        &args.value("policies")?,
    )?;
    let selected = class_field(&scopes, |s: &PlannerScopeResolution| &s.selected_scope)?;
    let selected = scope_data(&selected)?;
    let mut models: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in &selected.selected {
        if key.0 == MODEL
            && let Some(model) = selected.model(py, &key.1)
        {
            models.push(model);
        }
    }
    let overrides = args.value("overrides")?;
    let typed = type_cursor_overrides(&native_field(&overrides, "cursor_overrides")?, &models)?;
    Ok((
        scopes,
        replaced(&overrides, &[("cursor_overrides", typed)])?,
    ))
}

/// Execution and stale-warning change results for the inspected scopes.
fn change_results<'py>(
    project: &Bound<'py, PyAny>,
    (scopes, snapshot, identities): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    overrides: &Bound<'py, PyAny>,
    policies: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = project.py();
    let versions = class_field(identities, |i: &PlannerIdentityContext| {
        &i.version_identities
    })?;
    let changes = detect_changes(
        (
            project,
            &class_field(scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?,
            snapshot,
        ),
        native_field(overrides, "full_refresh")?.is_truthy()?,
        (
            native_field(&versions, "model_version_hashes")?,
            native_field(&versions, "model_metadata_jsons")?,
        ),
    )?;
    if !native_field(policies, "selection_diagnostics")?.is_truthy()? {
        let empty = native(
            py,
            RecordId::PlannerChangeResults,
            &[
                ("models", PyDict::new(py).into_any()),
                ("functions", PyDict::new(py).into_any()),
            ],
        )?;
        return Ok((changes, empty));
    }
    let stale = detect_stale_warning_changes((project, scopes), snapshot, (identities, &changes))?;
    Ok((changes, stale))
}

/// Redirect unselected upstreams to their `migrate_from` relation in the deferred target.
fn deferred_migration_origins<'py>(
    args: &KeywordArguments<'py>,
    scopes: &Bound<'py, PyAny>,
    deferral: &Bound<'py, PyAny>,
) -> PyResult<Vec<(String, String)>> {
    let locations = native_field(deferral, "deferred_locations")?;
    let relations = native_field(deferral, "deferred_relations")?;
    if locations.is_none() || relations.is_none() {
        return Ok(Vec::new());
    }
    defer_to_migration_origins(
        (
            &args.value("project")?,
            &args.value("adapter")?,
            &args.value("connection")?,
        ),
        (
            locations.downcast::<PyDict>()?,
            relations.downcast::<PyDict>()?,
        ),
        &unselected_upstream_models(scopes)?,
    )
}

/// The plan phases after warehouse inspection, run inside the inspection catalog.
fn planned_output<'py>(
    args: &KeywordArguments<'py>,
    runtime: &Bound<'py, PyAny>,
    (scopes, overrides): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    test_planning: &SqlTestPlanning,
) -> PyResult<Bound<'py, PyAny>> {
    let project = args.value("project")?;
    let py = project.py();
    let deferral = args.value("deferral")?;
    let policies = args.value("policies")?;
    let origin_failures = deferred_migration_origins(args, scopes, &deferral)?;
    let warehouse = gather_planner_warehouse_state(runtime, scopes, overrides, &deferral)?;
    let started = Instant::now();
    let identities = build_planner_identity_context(
        &project,
        scopes,
        native_field(&policies, "selection_diagnostics")?.extract()?,
    )?;
    check_selected_scope_buildability(runtime, scopes, &warehouse, &deferral)?;
    let snapshot = warehouse_field(&warehouse, |warehouse| &warehouse.snapshot)?;
    let (changes, stale_warning_changes) = change_results(
        &project,
        (scopes, &snapshot, &identities),
        overrides,
        &policies,
    )?;
    let inspection_scope = class_field(scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?;
    let resolved_actions = resolve_model_actions(&inspection_scope, &changes)?;
    let source_freshness = build_planner_source_freshness_result(
        (&project, &inspection_scope),
        (&args.value("adapter")?, &args.value("connection")?),
        &warehouse_field(&warehouse, |warehouse| &warehouse.inspection_relations)?,
        &native_field(&snapshot, "source_freshness_state_schemas")?,
    )?;
    let reconciliation = reconcile_execution_changes(
        (&inspection_scope, &snapshot),
        &identities,
        &resolved_actions,
        &changes,
    )?;
    let entries = build_planner_entry_results(
        (runtime, &warehouse),
        (overrides, &deferral),
        (&inspection_scope, &reconciliation),
        &source_freshness,
    )?;
    let planned_sql_tests = test_planning.result(py)?.into_bound(py);
    let output = base_plan_output(
        PlanOutputContext {
            runtime,
            warehouse: &warehouse,
            identities: &identities,
            overrides,
        },
        PlannedModelEntries {
            execution_scope: &inspection_scope,
            reconciliation: &reconciliation,
            entries,
            source_freshness: &source_freshness,
        },
        &planned_sql_tests,
    )?;
    let output = add_storage_policies(
        output,
        StoragePolicyContext {
            runtime,
            warehouse: &warehouse,
            scopes,
            policies: &policies,
        },
    )?;
    let output = add_plan_warnings(
        output,
        PlanWarningContext {
            scopes,
            warehouse: &warehouse,
            identities: &identities,
            stale_warning_changes: &stale_warning_changes,
        },
        (&source_freshness, &policies),
    )?;
    let output =
        add_deferred_origin_notes(output, (&project, scopes), (&deferral, &origin_failures))?;
    let plan_output = add_plan_metadata(output, &source_freshness, &policies)?.build(py)?;
    let on_progress = args.value("on_progress")?;
    if !on_progress.is_none() {
        on_progress.call1((format!(
            "Generated plan. ({:.2}s)",
            started.elapsed().as_secs_f64()
        ),))?;
    }
    Ok(plan_output)
}

/// Build the execution plan for one selection against live warehouse state.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn build_execution_plan<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_execution_plan",
        kwargs,
        (
            &[
                "project",
                "adapter",
                "connection",
                "selection",
                "overrides",
                "deferral",
                "policies",
            ],
            &["on_progress", "project_config", "local_config"],
        ),
    )?;
    let project = args.value("project")?;
    let adapter = args.value("adapter")?;
    let connection = args.value("connection")?;
    let runtime = Bound::new(
        py,
        PlannerRuntime {
            project: project.clone().unbind(),
            adapter: adapter.clone().unbind(),
            connection: connection.clone().unbind(),
            project_config: args.value("project_config")?.unbind(),
            local_config: args.value("local_config")?.unbind(),
            on_progress: args.value("on_progress")?.unbind(),
            invocation_time: utc_now(py)?.unbind(),
        },
    )?
    .into_any();
    let (scopes, overrides) = scopes_and_cursor_overrides(&args)?;
    let inspection = class_field(&scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?;
    let test_planning = start_planning(
        py,
        PlanningRequest {
            project: project.clone().unbind(),
            adapter: adapter.clone().unbind(),
            selected_keys: native_field(&inspection, "selected_keys")?.unbind(),
            sql_test_selection: native_field(&inspection, "sql_test_selection")?.unbind(),
        },
        native_field(&args.value("policies")?, "plan_sql_tests")?.is_truthy()?,
    )?;
    let planned = (|| {
        let catalog = call_with(
            &host::entry(py, INSPECTION_CATALOG)?,
            &[("adapter", adapter), ("connection", connection)],
        )?;
        within(&catalog, |_| {
            planned_output(&args, &runtime, (&scopes, &overrides), test_planning.get())
        })
    })();
    let joined = test_planning.get().result(py);
    let planned = planned?;
    joined?;
    Ok(planned)
}

/// Add the execution plan entry point.
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_execution_plan, module)?)
}
