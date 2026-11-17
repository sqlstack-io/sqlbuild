//! Top-level planner orchestration producing an execution plan.

use crate::bindings::_helpers::planner_phases::timings::ASSEMBLING;
use crate::bindings::_helpers::planner_phases::timings::BUILDING_ENTRIES;
use crate::bindings::_helpers::planner_phases::timings::DETECTING_CHANGES;
use crate::bindings::_helpers::planner_phases::timings::READING_WAREHOUSE;
use crate::bindings::_helpers::planner_phases::timings::SOURCE_FRESHNESS;
use crate::bindings::_helpers::planner_phases::timings::TEST_FIXTURES;
use crate::bindings::_helpers::planner_phases::timings::record;
use crate::bindings::_helpers::planner_phases::timings::timed;
use sqlbuild_analysis::planner_scope::constants::MODEL;

use crate::bindings::_helpers::planner_changes::actions::resolve_model_actions;
use crate::bindings::_helpers::planner_changes::detect::detect_changes;
use crate::bindings::_helpers::planner_entries::test_planning::PlanningRequest;
use crate::bindings::_helpers::planner_entries::test_planning::SqlTestPlanning;
use crate::bindings::_helpers::planner_entries::test_planning::start_planning;
use crate::bindings::_helpers::planner_entries::values::native;
use crate::bindings::_helpers::planner_entries::values::replaced;
use crate::bindings::_helpers::planner_phases::buildability::check_selected_scope_buildability;
use crate::bindings::_helpers::planner_phases::contexts::class_field;
use crate::bindings::_helpers::planner_phases::deferred_origins::defer_to_migration_origins;
use crate::bindings::_helpers::planner_phases::execution_changes::build_planner_entry_results;
use crate::bindings::_helpers::planner_phases::execution_changes::reconcile_execution_changes;
use crate::bindings::_helpers::planner_phases::identities::build_planner_identity_context;
use crate::bindings::_helpers::planner_phases::identities::detect_stale_warning_changes;
use crate::bindings::_helpers::planner_phases::models::PlanOutputContext;
use crate::bindings::_helpers::planner_phases::models::PlanWarningContext;
use crate::bindings::_helpers::planner_phases::models::PlannedModelEntries;
use crate::bindings::_helpers::planner_phases::models::PlannerIdentityContext;
use crate::bindings::_helpers::planner_phases::models::PlannerRuntime;
use crate::bindings::_helpers::planner_phases::models::PlannerScopeResolution;
use crate::bindings::_helpers::planner_phases::models::StoragePolicyContext;
use crate::bindings::_helpers::planner_phases::output_assembly::add_deferred_origin_notes;
use crate::bindings::_helpers::planner_phases::output_assembly::add_plan_metadata;
use crate::bindings::_helpers::planner_phases::output_assembly::add_plan_warnings;
use crate::bindings::_helpers::planner_phases::output_assembly::add_storage_policies;
use crate::bindings::_helpers::planner_phases::output_assembly::base_plan_output;
use crate::bindings::_helpers::planner_phases::output_assembly::unselected_upstream_models;
use crate::bindings::_helpers::planner_phases::runtime::warehouse_field;
use crate::bindings::_helpers::planner_phases::scopes::resolve_planner_scopes;
use crate::bindings::_helpers::planner_phases::warehouse_state::gather_planner_warehouse_state;
use crate::bindings::_helpers::planner_resolution::cursor_overrides::type_cursor_overrides;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::planner_warehouse::reads::PlannerReads;
use crate::bindings::_helpers::planner_warehouse::source_freshness::build_planner_source_freshness_result;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_command::main::utc_now::utc_now;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use std::time::Instant;

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
            &Dialect::passed(args.value("dialect")?)?,
            &PlannerReads::passed(args.value("reads")?)?,
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
    let warehouse = timed(READING_WAREHOUSE, || {
        gather_planner_warehouse_state(runtime, scopes, overrides, &deferral)
    })?;
    let started = Instant::now();
    let snapshot = warehouse_field(&warehouse, |warehouse| &warehouse.snapshot)?;
    let changes_started = Instant::now();
    let identities = build_planner_identity_context(
        &project,
        (scopes, &snapshot),
        native_field(&policies, "selection_diagnostics")?.extract()?,
    )?;
    check_selected_scope_buildability(runtime, scopes, &warehouse, &deferral)?;
    let (changes, stale_warning_changes) = change_results(
        &project,
        (scopes, &snapshot, &identities),
        overrides,
        &policies,
    )?;
    let inspection_scope = class_field(scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?;
    let resolved_actions = resolve_model_actions(&inspection_scope, &changes)?;
    record(DETECTING_CHANGES, changes_started.elapsed().as_secs_f64());
    let source_freshness = timed(SOURCE_FRESHNESS, || {
        build_planner_source_freshness_result(
            (&project, &inspection_scope),
            (
                &Dialect::passed(args.value("dialect")?)?,
                &PlannerReads::passed(args.value("reads")?)?,
            ),
            &warehouse_field(&warehouse, |warehouse| &warehouse.inspection_relations)?,
            &native_field(&snapshot, "source_freshness_state_schemas")?,
        )
    })?;
    let entries_started = Instant::now();
    let reconciliation = reconcile_execution_changes(
        (&inspection_scope, &snapshot),
        &identities,
        &resolved_actions,
        &changes,
    )?;
    let entries = build_planner_entry_results(
        (runtime, &warehouse),
        overrides,
        (&inspection_scope, &reconciliation),
        &source_freshness,
    )?;
    record(BUILDING_ENTRIES, entries_started.elapsed().as_secs_f64());
    let planned_sql_tests = timed(TEST_FIXTURES, || test_planning.result(py))?.into_bound(py);
    let assembling_started = Instant::now();
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
    record(ASSEMBLING, assembling_started.elapsed().as_secs_f64());
    let on_progress = args.value("on_progress")?;
    if !on_progress.is_none() {
        on_progress.call1((format!(
            "Generated plan. ({:.2}s)",
            started.elapsed().as_secs_f64()
        ),))?;
    }
    Ok(plan_output)
}

/// Build the execution plan for one selection through the plan's warehouse reads.
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
                "dialect",
                "reads",
                "selection",
                "overrides",
                "deferral",
                "policies",
            ],
            &["on_progress", "project_config", "local_config"],
        ),
    )?;
    let project = args.value("project")?;
    let dialect = Dialect::passed(args.value("dialect")?)?;
    let reads = PlannerReads::passed(args.value("reads")?)?;
    let runtime = Bound::new(
        py,
        PlannerRuntime {
            project: project.clone().unbind(),
            dialect: dialect.clone().unbind(),
            reads: reads.clone().unbind(),
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
            dialect: dialect.unbind(),
            selected_keys: native_field(&inspection, "selected_keys")?.unbind(),
            sql_test_selection: native_field(&inspection, "sql_test_selection")?.unbind(),
        },
        native_field(&args.value("policies")?, "plan_sql_tests")?.is_truthy()?,
    )?;
    let planned = reads.get().reads().within_inspection(py, &mut || {
        planned_output(&args, &runtime, (&scopes, &overrides), test_planning.get())
    });
    let joined = test_planning.get().result(py);
    let planned = planned?;
    joined?;
    Ok(planned)
}

/// Add the execution plan entry point.
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_execution_plan, module)?)
}
