//! Prepare build execution, run the build pipeline and the post-build Python checks.

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_config::effective::models::EffectiveConcurrency;
use sqlbuild_core::build_command::main::render_run_context::render_run_context;
use sqlbuild_core::build_command::models::{RunContextFacts, SelectorFileFact};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::{BuildExecutionResult, BuildStatus, ExecutionStatus};

use crate::bindings::_helpers::boundary::python_calls::{call_with, keywords, within};
use crate::bindings::_helpers::build_command::invocation::BuildContext;
use crate::bindings::_helpers::build_command::lifecycle::{
    DirectPythonLifecycle, LifecycleInputs, prepare_direct_python_lifecycle,
};
use crate::bindings::_helpers::build_command::support::{
    boolean, display_text, emit, frozenset, joined, keyword_relay, line_writer, none, text, tuple,
};
use crate::bindings::_helpers::build_progress::build_callbacks::{
    BuildProgressCallbacks, batch_complete_callback,
};
use crate::bindings::_helpers::build_progress::progress_reporters::ConnectionProgressReporter;
use crate::bindings::_helpers::build_progress::progress_text::write_execution_header;
use crate::bindings::_helpers::check::check_core::{
    build_check_relation_targets, load_results_by_loader_name, record_python_run_state_results,
    relevant_check_functions,
};
use crate::bindings::_helpers::check::check_results::write_check_results;
use crate::bindings::_helpers::command_runtime::warehouses::describe_connection_warehouse;
use crate::bindings::_helpers::compile_driver::relation_targets::build_project_relation_targets;
use crate::bindings::_helpers::cost_telemetry::context::CostContext;
use crate::bindings::_helpers::cursor::overrides::{cursor_integer, cursor_timestamp};
use crate::bindings::_helpers::freshness::publication::append_freshness_records;
use crate::bindings::_helpers::graph::python_node_graph::python_node_graph;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::results::ReadResult;
use crate::bindings::_helpers::planner_resolution::cursor_overrides::type_cursor_overrides;
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::sql_test_command::test_command::timeout_record;
use crate::bindings::_helpers::sql_test_command::timeouts::{TimeoutKind, resolve_run_timeout};

const EXECUTION_COMMAND: &str = "sqb build";
const NODE_START: [&str; 2] = ["name", "resource_kind"];
const CONNECTION_TIMING: [&str; 2] = ["connection_count", "elapsed_seconds"];

/// Prepared callbacks, concurrency, cursors and Python lifecycle.
pub(crate) struct Preparation<'py> {
    pub(crate) callbacks: Bound<'py, PyAny>,
    pub(crate) concurrency: i64,
    connection_progress: Bound<'py, PyAny>,
    lifecycle: Bound<'py, DirectPythonLifecycle>,
    cursors: [Bound<'py, PyAny>; 4],
}

/// The build result and its finalized Python node results.
pub(crate) struct Outcome<'py> {
    pub(crate) result: BuildExecutionResult<Bound<'py, PyAny>>,
    pub(crate) executor_result: Bound<'py, PyAny>,
    pub(crate) python_results: Bound<'py, PyAny>,
}

fn count(value: &Bound<'_, PyAny>, name: &str) -> PyResult<usize> {
    native_field(value, name)?.len()
}

fn validated_date(py: Python<'_>, value: Option<Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    let Some(value) = value.filter(|value| value.is_instance_of::<pyo3::types::PyString>()) else {
        return Ok(None);
    };
    let datetime: Bound<'_, PyAny> = py.import("datetime")?.into_any();
    for kind in ["date", "datetime"] {
        match native_field(&datetime, kind)?.call_method1("fromisoformat", (&value,)) {
            Ok(_) => return Ok(Some(value.extract()?)),
            Err(error) if error.is_instance_of::<PyValueError>(py) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}

fn run_context_facts(
    context: &BuildContext<'_>,
    pipeline: &Bound<'_, PyAny>,
    concurrency: &EffectiveConcurrency,
) -> PyResult<RunContextFacts> {
    let py: Python<'_> = context.py();
    let project: Bound<'_, PyAny> = native_field(pipeline, "project")?;
    let plan: Bound<'_, PyAny> = native_field(pipeline, "plan_output")?;
    let discovered: Bound<'_, PyAny> = context.get("discovered_inputs")?;
    let selected_count: usize = count(&plan, "model_entries")?
        + count(&plan, "seed_entries")?
        + count(&plan, "function_entries")?
        + count(&plan, "source_load_entries")?
        + count(pipeline, "python_plan_entries")?;
    let mut loaded_sources: usize = 0;
    for source in native_field(&project, "sources")?.try_iter()? {
        if !native_field(&source?, "source_entry")?
            .getattr("loader")?
            .is_none()
        {
            loaded_sources += 1;
        }
    }
    let total_count: usize = count(&project, "models")?
        + count(&project, "seeds")?
        + count(&project, "functions")?
        + loaded_sources
        + count(&discovered, "task_functions")?
        + count(&discovered, "asset_functions")?;
    let resolved: Bound<'_, PyAny> = describe_connection_warehouse(
        py,
        discovered,
        native_field(&project, "effective_target_name")?,
        context.get("connection_config")?,
    )?;
    let warehouse: Bound<'_, PyAny> = native_field(&resolved, "warehouse")?;
    let warehouse: Option<(String, String)> = if warehouse.is_none() {
        None
    } else {
        Some((
            display_text(&warehouse)?.unwrap_or_else(|| "not set".to_owned()),
            native_field(&resolved, "source")?.str()?.extract()?,
        ))
    };
    let variables: Bound<'_, PyAny> = native_field(&project, "effective_vars")?;
    let variable = |name: &str| -> PyResult<Option<Bound<'_, PyAny>>> {
        let value: Bound<'_, PyAny> = mapping_get(&variables, name)?;
        Ok((!value.is_none()).then_some(value))
    };
    let mut selector_files: Vec<SelectorFileFact> = Vec::new();
    for file in native_field(&context.request, "selector_files")?.try_iter()? {
        let file: Bound<'_, PyAny> = file?;
        selector_files.push(SelectorFileFact {
            path: native_field(&file, "path")?.str()?.extract()?,
            selector_count: native_field(&file, "selector_count")?.extract()?,
        });
    }
    Ok(RunContextFacts {
        command: EXECUTION_COMMAND.to_owned(),
        run_id: display_text(&native_field(&project, "run_id")?)?,
        target: display_text(&native_field(&project, "effective_target_name")?)?,
        database: display_text(&native_field(&project, "effective_target_database")?)?,
        schema: display_text(&native_field(&project, "effective_target_schema")?)?,
        warehouse,
        concurrency: concurrency.limit,
        concurrency_source: concurrency.source.to_string(),
        full_refresh: context.request_flag("full_refresh")?,
        selected_count,
        total_count,
        date_vars: (
            validated_date(py, variable("start_date")?)?,
            validated_date(py, variable("end_date")?)?,
        ),
        selector_files,
    })
}

/// Write the resolved non-secret placement, mode and selection context.
pub(crate) fn write_run_context(
    context: &BuildContext<'_>,
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let concurrency: EffectiveConcurrency = context.concurrency()?;
    let facts: RunContextFacts = run_context_facts(context, pipeline, &concurrency)?;
    emit(
        &context.stream,
        &render_run_context(
            &facts,
            CliStyle {
                use_color: context.use_color,
            },
        ),
    )
}

fn cursors<'py>(
    context: &BuildContext<'py>,
    plan: &Bound<'py, PyAny>,
) -> PyResult<[Bound<'py, PyAny>; 4]> {
    let py: Python<'py> = context.py();
    let mut overrides: Bound<'py, PyAny> = native_field(plan, "cursor_overrides")?;
    if !overrides.is_truthy()? {
        overrides =
            type_cursor_overrides(&native_field(&context.request, "cursor_overrides")?, &[])?;
    }
    if !overrides.is_truthy()? {
        overrides = new_record(py, RecordId::CursorOverrides, (), None)?;
    }
    Ok([
        cursor_timestamp(&native_field(&overrides, "start_ts")?)?,
        cursor_timestamp(&native_field(&overrides, "end_ts")?)?,
        cursor_integer(&native_field(&overrides, "start_int")?)?,
        cursor_integer(&native_field(&overrides, "end_int")?)?,
    ])
}

/// Build callbacks, the execution header, the execution connection reporter and the Python lifecycle.
pub(crate) fn prepare_build_execution<'py>(
    context: &BuildContext<'py>,
    pipeline: &Bound<'py, PyAny>,
    providers: &Bound<'py, PyAny>,
) -> PyResult<Preparation<'py>> {
    let py: Python<'py> = context.py();
    let plan: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    let debug: bool = context.request_flag("debug")? || context.request_flag("json_output")?;
    let callbacks: Bound<'py, PyAny> = BuildProgressCallbacks::new(
        py,
        Some(keywords(
            py,
            &[
                ("plan", plan.clone()),
                ("use_color", boolean(py, context.use_color)),
                ("verbose", native_field(&context.request, "verbose")?),
                ("debug", boolean(py, debug)),
                (
                    "event_output_path",
                    native_field(&context.request, "event_output_path")?,
                ),
            ],
        )?),
    )
    .map(|mut value| {
        value.worker = context.worker.clone();
        value
    })
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let concurrency = context.concurrency()?.limit;
    if context.verbose {
        write_run_context(context, pipeline)?;
    } else {
        write_execution_header(
            py,
            Some(keywords(
                py,
                &[
                    ("stream", context.stream.clone()),
                    ("command", text(py, EXECUTION_COMMAND)),
                    ("target", none(py)),
                    ("concurrency", concurrency.into_pyobject(py)?.into_any()),
                    ("use_color", boolean(py, context.use_color)),
                ],
            )?),
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    }
    let connection_progress: Bound<'py, PyAny> = ConnectionProgressReporter::new(
        py,
        Some(keywords(
            py,
            &[
                ("adapter_name", context.get("adapter_name")?),
                ("stream", context.stream.clone()),
                (
                    "blank_line_before_start",
                    boolean(py, native_field(&plan, "source_load_entries")?.is_truthy()?),
                ),
                ("blank_line_after_complete", boolean(py, true)),
                ("use_color", boolean(py, context.use_color)),
            ],
        )?),
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let cursors: [Bound<'py, PyAny>; 4] = cursors(context, &plan)?;
    let inputs: LifecycleInputs<'py> = LifecycleInputs {
        cursors: cursors.clone(),
        on_node_start: keyword_relay(native_field(&callbacks, "on_node_start")?, &NODE_START)?,
        on_node_complete: native_field(&callbacks, "on_node_complete")?,
        providers: providers.clone(),
    };
    let lifecycle: Bound<'py, DirectPythonLifecycle> =
        prepare_direct_python_lifecycle(context, pipeline, &inputs)?;
    Ok(Preparation {
        callbacks,
        concurrency,
        connection_progress,
        lifecycle,
        cursors,
    })
}

fn verbose_callback<'py>(
    context: &BuildContext<'py>,
    callbacks: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if context.verbose {
        return native_field(callbacks, name);
    }
    Ok(none(context.py()))
}

/// Run the build pipeline, record eligible source freshness and finalize the Python lifecycle.
pub(crate) fn execute_build_plan<'py>(
    context: &BuildContext<'py>,
    pipeline: &Bound<'py, PyAny>,
    preparation: &Preparation<'py>,
    providers: &Bound<'py, PyAny>,
) -> PyResult<Outcome<'py>> {
    let py: Python<'py> = context.py();
    let plan: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    let project: Bound<'py, PyAny> = native_field(pipeline, "project")?;
    let adapter: Bound<'py, PyAny> = context.get("adapter")?;
    let connection_config: Bound<'py, PyAny> = context.get("connection_config")?;
    let callbacks: &Bound<'py, PyAny> = &preparation.callbacks;
    let lifecycle: &DirectPythonLifecycle = preparation.lifecycle.get();
    let [start_ts, end_ts, start_int, end_int] = &preparation.cursors;
    let request = |name: &str| native_field(&context.request, name);
    let runtime: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::BuildRuntimeParams)?,
        &[
            ("run_id", native_field(&project, "run_id")?),
            (
                "runtime_dir",
                joined(&context.get("effective_project_dir")?, &["target"])?,
            ),
            (
                "snapshots",
                context
                    .get("discovered_inputs")?
                    .getattr("project_config")?
                    .getattr("snapshots")?,
            ),
            (
                "allow_snapshot_schema_change",
                request("allow_snapshot_schema_change")?,
            ),
            ("run_tests", request("run_tests")?),
            ("run_audits", request("run_audits")?),
            ("fail_fast", request("fail_fast")?),
            (
                "max_concurrency",
                preparation.concurrency.into_pyobject(py)?.into_any(),
            ),
            ("loader_is_reload", request("reload_sources")?),
            ("start_cursor_ts", start_ts.clone()),
            ("end_cursor_ts", end_ts.clone()),
            ("start_cursor_int", start_int.clone()),
            ("end_cursor_int", end_int.clone()),
            ("use_color", boolean(py, context.use_color)),
            ("providers", providers.clone()),
            (
                "test_timeouts",
                timeout_record(
                    py,
                    &resolve_run_timeout(
                        (&project, &context.get("discovered_inputs")?),
                        request("test_timeout")?.extract()?,
                        TimeoutKind::Test,
                    )?,
                )?,
            ),
            (
                "project_relations",
                build_project_relation_targets(
                    py,
                    Some(keywords(
                        py,
                        &[("adapter", adapter.clone()), ("plan_output", plan.clone())],
                    )?),
                )
                .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?,
            ),
        ],
    )?;
    let connection: &Bound<'py, PyAny> = &preparation.connection_progress;
    let build_callbacks: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::BuildCallbacks)?,
        &[
            ("on_migration_progress", line_writer(&context.stream)?),
            (
                "on_node_start",
                keyword_relay(native_field(callbacks, "on_node_start")?, &NODE_START)?,
            ),
            (
                "on_node_complete",
                native_field(&preparation.lifecycle, "on_node_complete")?,
            ),
            (
                "on_sub_progress",
                native_field(callbacks, "on_sub_progress")?,
            ),
            ("on_batch_complete", batch_complete_callback(callbacks)?),
            (
                "on_connection_start",
                native_field(connection, "on_connection_start")?,
            ),
            (
                "on_connection_complete",
                keyword_relay(
                    native_field(connection, "on_connection_complete")?,
                    &CONNECTION_TIMING,
                )?,
            ),
            (
                "on_connection_error",
                keyword_relay(
                    native_field(connection, "on_connection_error")?,
                    &CONNECTION_TIMING,
                )?,
            ),
            (
                "on_scheduler_state",
                verbose_callback(context, callbacks, "on_scheduler_state")?,
            ),
            (
                "on_statement_complete",
                verbose_callback(context, callbacks, "on_statement_complete")?,
            ),
        ],
    )?;
    let customizations: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::BuildCustomizations)?,
        &[
            (
                "custom_materializations",
                native_field(pipeline, "custom_materializations")?,
            ),
            ("loader_functions", lifecycle.loader_functions(py)?),
        ],
    )?;
    let initial_state: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::BuildInitialState)?,
        &[
            ("precompleted_keys", lifecycle.precompleted_keys(py)?),
            ("initial_load_results", lifecycle.ingress_load_results(py)),
            ("initial_failed_keys", lifecycle.blocked_keys(py)?),
        ],
    )?;
    let result: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::RunBuildPipeline)?,
        &[
            ("plan", plan.clone()),
            ("connection_config", connection_config.clone()),
            ("adapter", adapter.clone()),
            ("settings", native_field(&project, "settings")?),
            ("runtime", runtime),
            ("callbacks", build_callbacks),
            ("customizations", customizations),
            ("initial_state", initial_state),
        ],
    )?;
    let native_result = BuildExecutionResult::read(&result)?;
    let successful_models: Vec<String> = native_result
        .model_results
        .iter()
        .filter(|model| model.status == ExecutionStatus::Success)
        .map(|model| model.model_name.clone())
        .collect();
    append_freshness_records(
        (&plan, &successful_models),
        &adapter,
        &connection_config,
        &native_field(&project, "run_id")?,
    )?;
    lifecycle.finalize(py)?;
    Ok(Outcome {
        result: native_result,
        executor_result: result,
        python_results: lifecycle.python_results(py)?,
    })
}

fn check_runtime<'py>(
    context: &BuildContext<'py>,
    pipeline: &Bound<'py, PyAny>,
    (connection, graph, checks): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &[Bound<'py, PyAny>]),
    providers: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let adapter: Bound<'py, PyAny> = context.get("adapter")?;
    let project: Bound<'py, PyAny> = native_field(pipeline, "project")?;
    let names: Vec<Bound<'py, PyAny>> = checks
        .iter()
        .map(|check| native_field(check, "name"))
        .collect::<PyResult<_>>()?;
    call_with(
        &host::entry(py, ExecutorHost::PythonNodeRuntime)?,
        &[
            ("adapter", adapter.clone()),
            ("connection_config", context.get("connection_config")?),
            ("connection", connection.clone()),
            ("run_id", native_field(&project, "run_id")?),
            ("target", native_field(&project, "effective_target_name")?),
            ("vars", native_field(&project, "effective_vars")?),
            (
                "is_reload",
                native_field(&context.request, "reload_sources")?,
            ),
            (
                "default_database",
                adapter.call_method0("default_database")?,
            ),
            ("default_schema", adapter.call_method0("default_schema")?),
            (
                "relation_targets",
                build_check_relation_targets(&adapter, pipeline, graph, frozenset(py, &names)?)?,
            ),
            (
                "project_relations",
                build_project_relation_targets(
                    py,
                    Some(keywords(
                        py,
                        &[
                            ("adapter", adapter),
                            ("plan_output", native_field(pipeline, "plan_output")?),
                        ],
                    )?),
                )
                .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?,
            ),
            ("providers", providers.clone()),
        ],
    )
}

fn names_of<'py>(values: &Bound<'py, PyAny>, attribute: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    values
        .try_iter()?
        .map(|value| native_field(&value?, attribute))
        .collect()
}

/// Execute relevant Python check functions after a successful build.
pub(crate) fn run_post_build_python_checks<'py>(
    context: &BuildContext<'py>,
    pipeline: &Bound<'py, PyAny>,
    (outcome, preparation): (&Outcome<'py>, &Preparation<'py>),
    providers: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let result = &outcome.result;
    if result.status != BuildStatus::Success {
        return tuple(py, &[]);
    }
    let discovered: Bound<'py, PyAny> = context.get("discovered_inputs")?;
    let plan: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    let load_results: Bound<'py, PyAny> = native_field(&outcome.executor_result, "load_results")?;
    let graph: Bound<'py, PyAny> = python_node_graph(&discovered)?;
    let by_loader = || -> PyResult<Bound<'py, PyAny>> {
        Ok(
            load_results_by_loader_name(&native_field(&plan, "source_map")?, &load_results)?
                .into_any(),
        )
    };
    let mut dependencies: Vec<Bound<'py, PyAny>> = names_of(&outcome.python_results, "node_name")?;
    for name in by_loader()?.try_iter()? {
        dependencies.push(name?);
    }
    let completed: Vec<Bound<'py, PyAny>> = result
        .model_results
        .iter()
        .map(|model| model.model_name.as_str())
        .chain(
            result
                .seed_results
                .iter()
                .map(|seed| seed.seed_name.as_str()),
        )
        .chain(
            result
                .load_results
                .iter()
                .map(|load| load.source_name.as_str()),
        )
        .map(|name| text(py, name))
        .collect();
    let checks: Vec<Bound<'py, PyAny>> = relevant_check_functions(
        &discovered,
        &graph,
        native_field(&context.request, "exclude")?,
        (&frozenset(py, &dependencies)?, &frozenset(py, &completed)?),
    )?;
    if checks.is_empty() {
        return tuple(py, &[]);
    }
    let adapter: Bound<'py, PyAny> = context.get("adapter")?;
    let connection: Bound<'py, PyAny> =
        adapter.call_method1("connect", (context.get("connection_config")?,))?;
    let executed: PyResult<Bound<'py, PyAny>> = (|| {
        let run_state: Bound<'py, PyAny> =
            host::entry(py, ExecutorHost::PythonNodeRunState)?.call0()?;
        record_python_run_state_results(
            &discovered,
            &run_state,
            (&outcome.python_results, Some(&load_results)),
            Some(&native_field(&plan, "source_map")?),
        )?;
        let callbacks: Bound<'py, PyAny> = call_with(
            &host::entry(py, ExecutorHost::PythonCheckCallbacks)?,
            &[(
                "on_check_complete",
                native_field(&preparation.callbacks, "write_execution_event")?,
            )],
        )?;
        call_with(
            &host::entry(py, ExecutorHost::ExecutePythonChecks)?,
            &[
                ("check_functions", tuple(py, &checks)?),
                ("python_graph", graph.clone()),
                ("upstream_python_results", outcome.python_results.clone()),
                ("upstream_load_results", load_results.clone()),
                ("upstream_load_results_by_loader_name", by_loader()?),
                (
                    "runtime",
                    check_runtime(context, pipeline, (&connection, &graph, &checks), providers)?,
                ),
                ("run_state", run_state),
                ("callbacks", callbacks),
            ],
        )
    })();
    let closed: PyResult<Bound<'py, PyAny>> = adapter.call_method1("close", (connection,));
    let check_results: Bound<'py, PyAny> = executed?;
    closed?;
    write_check_results(
        py,
        Some(keywords(
            py,
            &[
                ("stream", context.stream.clone()),
                ("results", check_results.clone()),
                ("use_color", boolean(py, context.use_color)),
                ("check_functions", tuple(py, &checks)?),
                ("python_graph", graph),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    Ok(check_results)
}

/// Run the prepared build and its checks inside the run cost context.
pub(crate) fn execute_direct_build<'py>(
    context: &BuildContext<'py>,
    pipeline: &Bound<'py, PyAny>,
    providers: &Bound<'py, PyAny>,
) -> PyResult<(Preparation<'py>, Outcome<'py>, Bound<'py, PyAny>)> {
    let py: Python<'py> = context.py();
    let project: Bound<'py, PyAny> = native_field(pipeline, "project")?;
    let run_id: Bound<'py, PyAny> = native_field(&project, "run_id")?;
    let target_name: Bound<'py, PyAny> = native_field(&project, "effective_target_name")?;
    let resource_name: Bound<'py, PyAny> = if target_name.is_truthy()? {
        target_name
    } else {
        context.get("adapter_name")?
    };
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("run_id", &run_id)?;
    options.set_item("resource_type", "run")?;
    options.set_item("resource_name", &resource_name)?;
    options.set_item(
        "ledger_path",
        joined(
            &context.get("effective_project_dir")?,
            &[
                "target",
                "executions",
                &run_id.str()?.extract::<String>()?,
                "statements.jsonl",
            ],
        )?,
    )?;
    options.set_item("phase", "build")?;
    let scope: Bound<'py, PyAny> =
        Bound::new(py, CostContext::scope(py, Some(options))?)?.into_any();
    within(&scope, |_| {
        let preparation: Preparation<'py> = prepare_build_execution(context, pipeline, providers)?;
        let ran: PyResult<(Outcome<'py>, Bound<'py, PyAny>)> = (|| {
            let outcome: Outcome<'py> =
                execute_build_plan(context, pipeline, &preparation, providers)?;
            let checks_scope: Bound<'py, PyAny> = CostContext::resource_scope(
                py,
                Some(keywords(
                    py,
                    &[
                        ("resource_type", text(py, "run")),
                        ("resource_name", resource_name.clone()),
                        ("phase", text(py, "post_build_checks")),
                    ],
                )?),
            )
            .and_then(|scope| Bound::new(py, scope))?
            .into_any();
            let checks: Bound<'py, PyAny> = within(&checks_scope, |_| {
                run_post_build_python_checks(context, pipeline, (&outcome, &preparation), providers)
            })?;
            Ok((outcome, checks))
        })();
        let closed: PyResult<Bound<'py, PyAny>> = preparation.callbacks.call_method0("close");
        let (outcome, checks) = ran?;
        closed?;
        Ok((preparation, outcome, checks))
    })
}
