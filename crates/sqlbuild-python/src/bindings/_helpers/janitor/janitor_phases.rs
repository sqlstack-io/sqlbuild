//! `sqb janitor` phases: compile, connect, inspect and plan, clean up and close.

use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost};

use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::native_host::main::connection_failure::connection_failure;
use sqlbuild_python_compile::compile_driver::main::setup::build_compiled_project::build_compiled_project;
use std::time::Instant;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods};

use crate::bindings::_helpers::command_runtime::invocation::resolve_adapter_connection_context;
use crate::bindings::_helpers::janitor::janitor_facts::Warehouse;
use crate::bindings::_helpers::janitor::janitor_planning::{
    JanitorSettings, RelationScope, build_plan,
};
use crate::bindings::_helpers::janitor::janitor_usage::{UsageSettings, check_janitor_usage};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::build_progress::main::transient_status::status_reporter::status_reporter;
use sqlbuild_python_base::build_progress::models::TransientStatusReporter;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::command_runtime::main::resolve_concurrency::resolve_concurrency;
use sqlbuild_python_output::build_progress::models::ConnectionProgressReporter;
use sqlbuild_python_output::build_progress::models::PlanningProgressReporter;

const UNKNOWN_VIEW_CODE: &str = "C503";
const UNKNOWN_VIEW_HELP: &str =
    "Run sqb janitor without --drop-old-name-view to list compatibility views.";
const CANDIDATE_FIELDS: [&str; 5] = [
    "candidates",
    "archive_deletion_candidates",
    "query_diff_artifact_candidates",
    "direct_state_prune_candidates",
    "review_schema_candidates",
];

/// The compiled project, its adapter and the active target connection settings.
pub(crate) struct JanitorCompiled<'py> {
    pub(crate) adapter_name: Bound<'py, PyAny>,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) connection_config: Bound<'py, PyAny>,
}

/// Where human output goes: stderr when stdout carries `--json`, otherwise stdout.
pub(crate) fn output_stream<'py>(invocation: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let machine_output: bool = native_field(invocation, "machine_output")?.is_truthy()?;
    invocation
        .py()
        .import("sys")?
        .getattr(if machine_output { "stderr" } else { "stdout" })
}

fn use_color(invocation: &Bound<'_, PyAny>) -> PyResult<bool> {
    native_field(invocation, "use_color")?.extract()
}

/// `OperationLifecycle(operation_kind=..., operation_name=...)`.
fn lifecycle<'py>(py: Python<'py>, (kind, name): (&str, &str)) -> PyResult<Bound<'py, PyAny>> {
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("operation_kind", kind)?;
    options.set_item("operation_name", name)?;
    host_entry(py, ExecutorHost::OperationLifecycle)?.call((), Some(&options))
}

/// Compile for the janitor or `--as` target and resolve the active target connection.
pub(crate) fn compile_janitor_project<'py>(
    invocation: &Bound<'py, PyAny>,
) -> PyResult<JanitorCompiled<'py>> {
    let py: Python<'py> = invocation.py();
    let context: Bound<'py, PyAny> = resolve_adapter_connection_context(
        &native_field(invocation, "discovered_inputs")?,
        &native_field(invocation, "effective_project_dir")?,
        (
            native_field(invocation, "selected_target")?,
            py.None().into_bound(py),
        ),
        true,
    )?;
    let started: Instant = Instant::now();
    let status: Bound<'py, PyAny> = Bound::new(
        py,
        PlanningProgressReporter::new(py, &output_stream(invocation)?, use_color(invocation)?)?,
    )?
    .into_any();
    let as_target: Bound<'py, PyAny> = native_field(invocation, "as_target")?;
    let target: Bound<'py, PyAny> = if as_target.is_none() {
        native_field(invocation, "selected_target")?
    } else {
        as_target
    };
    let project: Bound<'py, PyAny> =
        within(&lifecycle(py, ("project", "project_compile"))?, |_| {
            status.call_method1("on_progress", ("Compiling project...",))?;
            let project: Bound<'py, PyAny> = build_compiled_project(
                py,
                Some(keywords(
                    py,
                    &[
                        (
                            "discovered_inputs",
                            native_field(invocation, "discovered_inputs")?,
                        ),
                        ("adapter", native_field(&context, "adapter")?),
                        ("selected_target", target),
                        (
                            "resolved_connection",
                            native_field(&context, "connection_config")?,
                        ),
                    ],
                )?),
            )?;
            status.call_method1(
                "on_progress",
                (format!(
                    "Compiled project. ({:.2}s)",
                    started.elapsed().as_secs_f64()
                ),),
            )?;
            Ok(project)
        })?;
    Ok(JanitorCompiled {
        adapter_name: native_field(&context, "adapter_name")?,
        adapter: native_field(&context, "adapter")?,
        project,
        connection_config: native_field(&context, "connection_config")?,
    })
}

fn connection_timing(progress: &Bound<'_, PyAny>, method: &str, started: Instant) -> PyResult<()> {
    let options: Bound<'_, PyDict> = PyDict::new(progress.py());
    options.set_item("connection_count", 1)?;
    options.set_item("elapsed_seconds", started.elapsed().as_secs_f64())?;
    progress.call_method(method, (), Some(&options))?;
    Ok(())
}

/// Connect to the janitor target warehouse.
pub(crate) fn connect_janitor_warehouse<'py>(
    invocation: &Bound<'py, PyAny>,
    compiled: &JanitorCompiled<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.py();
    let progress: Bound<'py, PyAny> = Bound::new(
        py,
        ConnectionProgressReporter::new(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter_name", compiled.adapter_name.clone()),
                    ("stream", output_stream(invocation)?),
                    (
                        "use_color",
                        PyBool::new(py, use_color(invocation)?)
                            .to_owned()
                            .into_any(),
                    ),
                ],
            )?),
        )?,
    )?
    .into_any();
    let started: Instant = Instant::now();
    progress.call_method1("on_connection_start", (1,))?;
    let connected: PyResult<Bound<'py, PyAny>> = within(
        &lifecycle(py, ("janitor", "janitor_target_connection"))?,
        |_| {
            compiled
                .adapter
                .call_method1("connect", (&compiled.connection_config,))
        },
    );
    match connected {
        Ok(connection) => {
            connection_timing(&progress, "on_connection_complete", started)?;
            Ok(connection)
        }
        Err(error) => {
            connection_timing(&progress, "on_connection_error", started)?;
            Err(connection_failure(&compiled.adapter, &error))
        }
    }
}

/// Close the janitor warehouse connection.
pub(crate) fn close_janitor_warehouse(
    compiled: &JanitorCompiled<'_>,
    connection: &Bound<'_, PyAny>,
) -> PyResult<()> {
    compiled.adapter.call_method1("close", (connection,))?;
    Ok(())
}

fn candidate_count(plan: &Bound<'_, PyAny>) -> PyResult<usize> {
    let mut count: usize = native_field(plan, "old_name_views")?
        .getattr("drops")?
        .len()?;
    for field in CANDIDATE_FIELDS {
        count += native_field(plan, field)?.len()?;
    }
    Ok(count)
}

fn unknown_view_error(py: Python<'_>, requests: &Bound<'_, PyAny>) -> PyResult<PyErr> {
    let names: Vec<String> = requests.extract()?;
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("code", UNKNOWN_VIEW_CODE)?;
    options.set_item("help", UNKNOWN_VIEW_HELP)?;
    let error: Bound<'_, PyAny> = error_class(py, ErrorId::CliUser)?.call(
        (format!(
            "no compatibility view is recorded at {}",
            names.join(", ")
        ),),
        Some(&options),
    )?;
    Ok(PyErr::from_value(error))
}

/// Build the janitor execution plan from warehouse facts.
pub(crate) fn build_janitor_execution_plan<'py>(
    (invocation, settings): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    compiled: &JanitorCompiled<'py>,
    connection: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.py();
    let started: Instant = Instant::now();
    let status: Bound<'py, PyAny> =
        status_reporter(&output_stream(invocation)?, use_color(invocation)?)?;
    status.call_method1("start", ("Inspecting warehouse state...",))?;
    let janitor: Bound<'py, PyAny> = native_field(invocation, "discovered_inputs")?
        .getattr("project_config")?
        .getattr("janitor")?;
    let plan: Bound<'py, PyAny> = within(
        &lifecycle(py, ("janitor", "janitor_candidate_planning"))?,
        |lifecycle| {
            let catalog: Bound<'py, PyAny> = call_with(
                &host_entry(py, AdapterHost::OpenInspectionCatalog)?,
                &[
                    ("adapter", compiled.adapter.clone()),
                    ("connection", connection.clone()),
                ],
            )?;
            let plan: Bound<'py, PyAny> = within(&catalog, |_| {
                build_plan(
                    (
                        &compiled.project,
                        &Warehouse {
                            adapter: compiled.adapter.clone(),
                            connection: connection.clone(),
                        },
                    ),
                    &JanitorSettings {
                        retention_days: native_field(settings, "retention_days")?.extract()?,
                        delete_tracked_only: native_field(&janitor, "delete_tracked_only")?
                            .is_truthy()?,
                        exclude_patterns: native_field(&janitor, "exclude_patterns")?.extract()?,
                        direct_mode: true,
                        state_history_versions: native_field(
                            settings,
                            "direct_state_history_versions",
                        )?
                        .extract()?,
                        archive_retention_days: native_field(settings, "archive_retention_days")?
                            .extract()?,
                        early_old_name_view_drops: native_field(settings, "drop_old_name_views")?
                            .extract()?,
                    },
                    &RelationScope {
                        scan_keys: Vec::new(),
                        protected_keys: Vec::new(),
                        reasons: Vec::new(),
                    },
                )
            })?;
            let metadata: Bound<'py, PyDict> = PyDict::new(py);
            metadata.set_item("item_count", candidate_count(&plan)?)?;
            let options: Bound<'py, PyDict> = PyDict::new(py);
            options.set_item("metadata", metadata)?;
            lifecycle.call_method("completed", (), Some(&options))?;
            Ok(plan)
        },
    )?;
    let has_candidates: bool = native_field(&plan, "archive_candidates")?.is_truthy()?;
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item(
        "message",
        format!(
            "Inspected warehouse state. ({:.2}s)",
            started.elapsed().as_secs_f64()
        ),
    )?;
    options.set_item("blank_line_after", !has_candidates)?;
    status.call_method("complete", (), Some(&options))?;
    let unknown: Bound<'py, PyAny> =
        native_field(&plan, "old_name_views")?.getattr("unknown_requests")?;
    if unknown.is_truthy()? {
        return Err(unknown_view_error(py, &unknown)?);
    }
    if !has_candidates {
        return Ok(plan);
    }
    check_janitor_usage(
        (&compiled.adapter, connection),
        (
            &plan,
            &Bound::new(
                py,
                TransientStatusReporter::new(
                    py,
                    &output_stream(invocation)?,
                    use_color(invocation)?,
                    true,
                )?,
            )?,
        ),
        &UsageSettings {
            window: native_field(settings, "in_use_window")?.extract()?,
            keep_in_use: native_field(settings, "keep_in_use")?.is_truthy()?,
        },
    )
}

/// Execute the cleanup plan on up to the command's concurrency, reporting to `progress`.
pub(crate) fn execute_janitor_cleanup<'py>(
    invocation: &Bound<'py, PyAny>,
    (compiled, connection): (&JanitorCompiled<'py>, &Bound<'py, PyAny>),
    (plan, progress): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.py();
    let concurrency: i64 = resolve_concurrency(
        &py.None().into_bound(py),
        &native_field(invocation, "discovered_inputs")?,
        &native_field(invocation, "selected_target")?,
    )?
    .limit;
    let connection_progress: Bound<'py, PyAny> = Bound::new(
        py,
        ConnectionProgressReporter::new(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter_name", compiled.adapter_name.clone()),
                    ("stream", output_stream(invocation)?),
                    (
                        "use_color",
                        PyBool::new(py, use_color(invocation)?)
                            .to_owned()
                            .into_any(),
                    ),
                ],
            )?),
        )?,
    )?
    .into_any();
    call_with(
        &host_entry(py, ExecutorHost::ExecuteJanitorPlan)?,
        &[
            ("plan", plan.clone()),
            ("adapter", compiled.adapter.clone()),
            ("connection", connection.clone()),
            ("progress", progress.clone()),
            ("connection_config", compiled.connection_config.clone()),
            ("concurrency", concurrency.into_pyobject(py)?.into_any()),
            ("connection_progress", connection_progress),
        ],
    )
}
