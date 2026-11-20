//! Compile the build plan, show it, gate it and write the compile target.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyDictMethods, PyErr, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_config::command_connection::constants::{
    CONNECTION_SOURCE_KEY, DBT_PROFILE_CONNECTION_SOURCE,
};
use sqlbuild_core::build_command::constants::{
    EXECUTION_LIMIT_BUILD_NOTE, MISSING_ORIGIN_BUILD_HELP, RETENTION_DECREASE_BUILD_HELP,
    SNAPSHOT_FULL_REFRESH_BUILD_HELP, TABLE_TYPE_DOWNGRADE_BUILD_HELP,
};
use sqlbuild_core::build_command::models::PlanSafetyGates;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::build_command::defer_clone::run_defer_clone_boundary_prephase;
use crate::bindings::_helpers::build_command::invocation::BuildContext;
use crate::bindings::_helpers::build_command::policies::enforce_plan_policies;
use crate::bindings::_helpers::build_command::policies::model_limit;
use crate::bindings::_helpers::command_runtime::audit_failures::resolve_ignore_audit_failures;
use crate::bindings::_helpers::compile_driver::plan_pipeline::planned_project;
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::build_command::main::boolean::boolean;
use sqlbuild_python_base::build_command::main::emit::emit;
use sqlbuild_python_base::build_command::main::joined::joined;
use sqlbuild_python_base::build_command::main::keyword_relay::keyword_relay;
use sqlbuild_python_base::build_command::main::none::none;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_compile::manifest::main::build_manifest::build_manifest;
use sqlbuild_python_front::compile_attachment::main::build_effective_connection_config::build_effective_connection_config;
use sqlbuild_python_output::build_progress::main::with_verbose_inspection::with_verbose_inspection;
use sqlbuild_python_planner::plan_output::main::format_plan::format_plan;

const CONNECTION_TIMING: [&str; 2] = ["connection_count", "elapsed_seconds"];

/// Clone out-of-selection boundaries from the defer-clone origin before planning.
pub(crate) fn run_defer_clone_prephase(context: &BuildContext<'_>) -> PyResult<()> {
    let origin: Bound<'_, PyAny> = context.get("effective_defer_clone_from")?;
    if origin.is_none() {
        return Ok(());
    }
    run_defer_clone_boundary_prephase(context, origin)
}

fn request_options<'py>(context: &BuildContext<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let request: &Bound<'py, PyAny> = &context.request;
    let field = |name: &str| native_field(request, name);
    let should_load_sources: Bound<'py, PyAny> = context.get("should_load_sources")?;
    let python_selectors: bool =
        context.request_flag("include_python")? || should_load_sources.is_truthy()?;
    let resolver: Bound<'py, PyAny> = resolve_external_sql_reference_resolver(
        context.get("effective_project_dir")?,
        context.get("discovered_inputs")?,
    )?;
    new_record_with(
        py,
        RecordId::CompilePipelineOptions,
        &[
            ("no_sql_validation", field("no_sql_validation")?),
            ("no_cache", field("no_cache")?),
            ("verbose", field("verbose")?),
            ("selected_target", field("selected_target")?),
            ("defer_to", field("defer_to")?),
            ("defer_sources_to", field("defer_sources_to")?),
            ("plan_from", field("plan_from")?),
            ("cursor_overrides", field("cursor_overrides")?),
            ("select", field("select")?),
            ("exclude", field("exclude")?),
            ("full_refresh", field("full_refresh")?),
            ("auto_load_sources", should_load_sources),
            ("reload_sources", field("reload_sources")?),
            ("connection_config", context.get("connection_config")?),
            ("cli_vars", field("cli_vars")?),
            ("external_sql_reference_resolver", resolver),
            (
                "resolve_python_run_selectors",
                boolean(py, python_selectors),
            ),
            ("max_microbatches", field("max_microbatches")?),
            ("concurrency", field("concurrency")?),
            ("selection_diagnostics", field("selection_diagnostics")?),
            ("plan_sql_tests", field("run_tests")?),
            (
                "ignore_audit_failures",
                boolean(
                    py,
                    resolve_ignore_audit_failures(
                        request,
                        &context.get("discovered_inputs")?,
                        &field("selected_target")?,
                    )?,
                ),
            ),
            ("cli_ignore_audit_failures", field("ignore_audit_failures")?),
            ("job_settings", field("job_settings")?),
            ("accepts_unit_test_selectors", boolean(py, true)),
        ],
    )
}

/// Run the compile pipeline with planning and connection progress.
pub(crate) fn compile_build_plan<'py>(context: &BuildContext<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    emit(&context.stream, "\n")?;
    let connection: Bound<'py, PyAny> = context.get("connection_progress")?;
    let hooks: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::ConnectionHooks)?,
        &[
            (
                "on_progress",
                context.get("planning_progress")?.getattr("on_progress")?,
            ),
            (
                "on_connection_start",
                native_field(&connection, "on_connection_start")?,
            ),
            (
                "on_connection_complete",
                keyword_relay(
                    native_field(&connection, "on_connection_complete")?,
                    &CONNECTION_TIMING,
                )?,
            ),
            (
                "on_connection_error",
                keyword_relay(
                    native_field(&connection, "on_connection_error")?,
                    &CONNECTION_TIMING,
                )?,
            ),
        ],
    )?;
    let verbose: bool = native_field(&context.request, "verbose")?.is_truthy()?;
    let options: Bound<'py, PyAny> = request_options(context)?;
    let held: Option<Bound<'py, PyAny>> = held_compile(context, &options)?;
    with_verbose_inspection(py, verbose, || {
        planned_project(
            (&context.get("discovered_inputs")?, &context.get("adapter")?),
            (&options, hooks),
            (held.as_ref(), None),
        )
    })
}

/// The compile a worker verified, when compiling again could not differ: no run variables, validation and cache on, no dbt.
fn held_compile<'py>(
    context: &BuildContext<'py>,
    options: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let held: Bound<'py, PyAny> = native_field(&context.request, "compiled_project")?;
    let discovered: Bound<'py, PyAny> = context.get("discovered_inputs")?;
    let project_config: Bound<'py, PyAny> = native_field(&discovered, "project_config")?;
    let dbt_manifest: Bound<'py, PyAny> =
        native_field(&native_field(&project_config, "dbt")?, "target_path")?;
    if held.is_none()
        || native_field(options, "cli_vars")?.is_truthy()?
        || native_field(options, "no_sql_validation")?.is_truthy()?
        || native_field(options, "no_cache")?.is_truthy()?
        || !dbt_manifest.is_none()
    {
        return Ok(None);
    }
    let connection: Bound<'py, PyDict> = build_effective_connection_config(
        &discovered,
        Some(native_field(options, "selected_target")?),
        None,
        false,
    )?
    .cast_into()?;
    let from_dbt_profile: bool = connection
        .get_item(CONNECTION_SOURCE_KEY)?
        .map(|source| source.eq(DBT_PROFILE_CONNECTION_SOURCE))
        .transpose()?
        .unwrap_or(false);
    Ok((!from_dbt_profile).then_some(held))
}

/// Write the formatted plan to stdout, or stderr for debug and JSON runs.
pub(crate) fn write_plan_text(
    context: &BuildContext<'_>,
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = context.py();
    let machine: bool = context.request_flag("debug")? || context.request_flag("json_output")?;
    let stream: Bound<'_, PyAny> =
        py.import("sys")?
            .getattr(if machine { "stderr" } else { "stdout" })?;
    let plan_text: String = format_plan(
        py,
        Some(keywords(
            py,
            &[
                ("plan", native_field(pipeline, "plan_output")?),
                (
                    "full_refresh",
                    native_field(&context.request, "full_refresh")?,
                ),
                ("use_color", boolean(py, context.use_color)),
                (
                    "python_plan_entries",
                    native_field(pipeline, "python_plan_entries")?,
                ),
                ("include_direct_freshness_diagnostics", boolean(py, false)),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
    .extract()?;
    emit(&stream, &format!("\n{plan_text}\n\n"))
}

/// Apply every plan safety gate with the build's allow flags.
pub(crate) fn enforce_build_plan_policies(
    context: &BuildContext<'_>,
    pipeline: &Bound<'_, PyAny>,
) -> Result<(), PyErr> {
    let py: Python<'_> = context.py();
    let gates: PlanSafetyGates = PlanSafetyGates {
        allow_missing_migration_origin: context.request_flag("allow_missing_migration_origin")?,
        allow_snapshot_full_refresh: context.request_flag("allow_snapshot_full_refresh")?,
        allow_table_type_downgrade: context.request_flag("allow_table_type_downgrade")?,
        allow_retention_decrease: context.request_flag("allow_retention_decrease")?,
        missing_origin_help: MISSING_ORIGIN_BUILD_HELP.to_owned(),
        snapshot_full_refresh_help: SNAPSHOT_FULL_REFRESH_BUILD_HELP.to_owned(),
        table_type_downgrade_help: TABLE_TYPE_DOWNGRADE_BUILD_HELP.to_owned(),
        retention_decrease_help: RETENTION_DECREASE_BUILD_HELP.to_owned(),
        execution_limit_note: EXECUTION_LIMIT_BUILD_NOTE.to_owned(),
    };
    let sys: Bound<'_, PyAny> = py.import("sys")?.into_any();
    enforce_plan_policies(
        &native_field(pipeline, "plan_output")?,
        (
            &gates,
            &context
                .get("discovered_inputs")?
                .getattr("project_config")?
                .getattr("snapshots")?,
        ),
        &model_limit(
            &context.get("effective_target_name")?,
            &context.get("execution_limits")?,
        )?,
        (
            &native_field(&sys, "stdin")?,
            &native_field(&sys, "stdout")?,
        ),
    )
}

/// Write the compile target artifacts with the optional manifest payload.
pub(crate) fn write_compile_target(
    context: &BuildContext<'_>,
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = context.py();
    let plan: Bound<'_, PyAny> = native_field(pipeline, "plan_output")?;
    let manifest: Bound<'_, PyAny> = if context.request_flag("manifest")? {
        build_manifest(Some(&keywords(
            py,
            &[
                ("project", native_field(pipeline, "project")?),
                ("plan_output", plan.clone()),
                (
                    "project_name",
                    context
                        .get("discovered_inputs")?
                        .getattr("project_config")?
                        .getattr("name")?,
                ),
                ("adapter_type", context.get("adapter_name")?),
                ("upstream_deps", native_field(&plan, "upstream_deps")?),
                ("downstream_deps", native_field(&plan, "downstream_deps")?),
            ],
        )?))
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
    } else {
        none(py)
    };
    sqlbuild_python_compile::compile_command::main::artifacts::write_compile_target::write_compile_target(
        py,
        Some(keywords(
            py,
            &[
                (
                    "target_dir",
                    joined(&context.get("effective_project_dir")?, &["target"])?,
                ),
                ("adapter", context.get("adapter")?),
                ("plan_output", plan),
                ("manifest", manifest),
            ],
        )?),
    )?;
    Ok(())
}
