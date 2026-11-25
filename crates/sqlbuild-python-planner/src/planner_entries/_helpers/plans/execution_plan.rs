//! Top-level planner orchestration producing an execution plan.

use crate::planner_phases::constants::ASSEMBLING;
use crate::planner_phases::constants::BUILDING_ENTRIES;
use crate::planner_phases::constants::DETECTING_CHANGES;
use crate::planner_phases::constants::READING_WAREHOUSE;
use crate::planner_phases::constants::SOURCE_FRESHNESS;
use crate::planner_phases::constants::TEST_FIXTURES;
use crate::planner_phases::main::timing::record_step::record_step;
use crate::planner_phases::main::timing::timed::timed;
use crate::planner_warehouse::models::PlannerReads;
use sqlbuild_analysis::planner_scope::constants::MODEL;
use sqlbuild_python_base::invocation_phases::constants::PLANNED_PHASE;
use sqlbuild_python_base::invocation_phases::main::phase_since::phase_since;
use sqlbuild_python_base::invocation_phases::models::InvocationPhase;
use sqlbuild_python_base::native_host::main::publish_invocation_phase::publish_invocation_phase;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_project::planner_scope::models::RetainedScope;

use crate::planner_changes::main::detect_changes::detect_changes;
use crate::planner_changes::main::resolve_model_actions::resolve_model_actions;
use crate::planner_entries::_helpers::entries::values::{native, replaced};
use crate::planner_entries::_helpers::plans::test_planning::SqlTestPlanning;
use crate::planner_phases::main::inputs::class_field::class_field;
use crate::planner_phases::main::inputs::gather_planner_warehouse_state::gather_planner_warehouse_state;
use crate::planner_phases::main::inputs::warehouse_field::warehouse_field;
use crate::planner_phases::main::outputs::add_deferred_origin_notes::add_deferred_origin_notes;
use crate::planner_phases::main::outputs::add_plan_metadata::add_plan_metadata;
use crate::planner_phases::main::outputs::add_plan_warnings::add_plan_warnings;
use crate::planner_phases::main::outputs::add_storage_policies::add_storage_policies;
use crate::planner_phases::main::outputs::base_plan_output::base_plan_output;
use crate::planner_phases::main::outputs::build_planner_identity_context::build_planner_identity_context;
use crate::planner_phases::main::outputs::check_selected_scope_buildability::check_selected_scope_buildability;
use crate::planner_phases::main::outputs::detect_stale_warning_changes::detect_stale_warning_changes;
use crate::planner_phases::main::outputs::plan_dynamic_refresh::plan_dynamic_refresh;
use crate::planner_phases::main::outputs::unselected_upstream_models::unselected_upstream_models;
use crate::planner_phases::main::rebuilds::build_planner_entry_results::build_planner_entry_results;
use crate::planner_phases::main::rebuilds::defer_to_migration_origins::defer_to_migration_origins;
use crate::planner_phases::main::rebuilds::reconcile_execution_changes::reconcile_execution_changes;
use crate::planner_phases::models::FullRefreshRequest;
use crate::planner_phases::models::PlannerIdentityContext;
use crate::planner_phases::models::{
    PlanOutputContext, PlanWarningContext, PlannedModelEntries, StoragePolicyContext,
};
use crate::planner_resolution::main::cursor::type_cursor_overrides::type_cursor_overrides;
use crate::planner_warehouse::main::sources::build_planner_source_freshness_result::build_planner_source_freshness_result;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::planner_scope::main::resolve_planner_scopes::resolve_planner_scopes;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use sqlbuild_python_project::planner_scope::models::PlannerScopeResolution;
use std::time::Instant;

/// Selected scopes, and the overrides with cursor overrides typed for the selected models.
pub(crate) fn scopes_and_cursor_overrides<'py>(
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
    let full_refresh: FullRefreshRequest = FullRefreshRequest::of(overrides)?;
    let changes = detect_changes(
        (
            project,
            &class_field(scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?,
            snapshot,
        ),
        (full_refresh.all, &full_refresh.functions),
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
pub(crate) fn planned_output<'py>(
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
    let audits_not_run = check_selected_scope_buildability(runtime, scopes, &warehouse, &deferral)?;
    let (changes, stale_warning_changes) = change_results(
        &project,
        (scopes, &snapshot, &identities),
        overrides,
        &policies,
    )?;
    let inspection_scope = class_field(scopes, |s: &PlannerScopeResolution| &s.inspection_scope)?;
    let resolved_actions = resolve_model_actions(&inspection_scope, &changes)?;
    record_step(DETECTING_CHANGES, changes_started.elapsed().as_secs_f64());
    let freshness_started = Instant::now();
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
    let freshness_seconds: f64 = freshness_started.elapsed().as_secs_f64();
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
    let entries = plan_dynamic_refresh(runtime, &inspection_scope, entries)?;
    record_step(BUILDING_ENTRIES, entries_started.elapsed().as_secs_f64());
    let planned_sql_tests = timed(TEST_FIXTURES, || test_planning.result(py))?.into_bound(py);
    let assembling_started = Instant::now();
    let output = base_plan_output(
        PlanOutputContext {
            runtime,
            warehouse: &warehouse,
            identities: &identities,
            overrides,
            audits_not_run: &audits_not_run,
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
    record_step(ASSEMBLING, assembling_started.elapsed().as_secs_f64());
    publish_invocation_phase(
        py,
        &InvocationPhase {
            count: Some(
                i64::try_from(
                    class_field(scopes, |s: &PlannerScopeResolution| &s.selected_scope)?
                        .cast::<RetainedScope>()?
                        .get()
                        .data
                        .selected
                        .len(),
                )
                .unwrap_or(i64::MAX),
            ),
            detail: Some(format!("source freshness {freshness_seconds:.2} s")),
            ..phase_since(PLANNED_PHASE, started)
        },
    )?;
    let on_progress = args.value("on_progress")?;
    if !on_progress.is_none() {
        on_progress.call1((format!(
            "Generated plan. ({:.2}s)",
            started.elapsed().as_secs_f64()
        ),))?;
    }
    Ok(plan_output)
}
