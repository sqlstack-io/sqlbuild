//! `sqb load`: run the selected source loaders and report each load.

use std::time::Instant;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyInt, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::load_command::main::load_counts::load_counts;
use sqlbuild_core::load_command::main::load_exit_code::load_exit_code;
use sqlbuild_core::load_command::main::render_empty_load::render_empty_load;
use sqlbuild_core::load_command::main::render_load_footer::render_load_footer;
use sqlbuild_core::load_command::main::render_load_plan::render_load_plan;
use sqlbuild_core::load_command::main::render_load_ready::render_load_ready;
use sqlbuild_core::load_command::models::{LoadCounts, LoadResultText};
use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost, UserCodeHost};

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::build_progress::progress_reporters::ConnectionProgressReporter;
use crate::bindings::_helpers::build_progress::progress_text::write_execution_header;
use crate::bindings::_helpers::command_runtime::adapters::resolve_cli_adapter;
use crate::bindings::_helpers::command_runtime::concurrency::resolve_concurrency;
use crate::bindings::_helpers::command_runtime::connection::resolve_project_connection_config;
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_runtime_config;
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_target_config;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::cursor::overrides::{cursor_integer, cursor_timestamp};
use crate::bindings::_helpers::load::load_progress::{
    LoadCallbackRelay, LoadProgressReporter, load_result_text,
};
use crate::bindings::_helpers::load::load_selection::{
    LoadProject, LoaderDefaults, loader_defaults, loader_write_schemas, target_vars,
};
use crate::bindings::_helpers::native_host::connections::connection_failure;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_resolution::cursor_overrides::type_cursor_overrides;
use crate::bindings::_helpers::planner_selection::unit_tests::reject_unit_test_selectors;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::references::source_relations::render_source_relation;
use crate::bindings::_helpers::result_output::documents::format_load_execution_json;
use crate::bindings::_helpers::result_output::documents::write_execution_json_output;
use crate::bindings::_helpers::result_output::event_writer::ExecutionEventWriter;

const LOADER_NODE_KEY: &str = "sqlbuild_loader_node";
const EXECUTION_COMMAND: &str = "sqb load";

/// The discovered project, selected loads and output context of one load.
struct Invocation<'py> {
    project_dir: Bound<'py, PyAny>,
    discovered: Bound<'py, PyAny>,
    selected: Vec<Bound<'py, PyAny>>,
    references: Vec<Bound<'py, PyAny>>,
    relation_sources: Vec<Bound<'py, PyAny>>,
    style: CliStyle,
    stream: Bound<'py, PyAny>,
}

/// The adapter, connection, runtime and providers prepared for execution.
struct Preparation<'py> {
    adapter_name: String,
    adapter: Bound<'py, PyAny>,
    connection_config: Bound<'py, PyAny>,
    runtime: (Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyAny>),
    concurrency: i64,
    cursor_overrides: Bound<'py, PyAny>,
    provider_session: Bound<'py, PyAny>,
    project_relations: Bound<'py, PyAny>,
}

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn write(stream: &Bound<'_, PyAny>, value: &str) -> PyResult<()> {
    stream.call_method1("write", (value,))?;
    stream.call_method0("flush")?;
    Ok(())
}

fn invocation<'py>(py: Python<'py>, request: &Bound<'py, PyAny>) -> PyResult<Invocation<'py>> {
    let project_dir: Bound<'py, PyAny> = match native_field(request, "project_dir")? {
        directory if !directory.is_none() => directory,
        _ => py.import("pathlib")?.getattr("Path")?.call_method0("cwd")?,
    };
    let discovered: Bound<'py, PyAny> = discover_project_inputs(
        py,
        Some(keywords(py, &[("project_dir", project_dir.clone())])?),
    )?;
    let select: Bound<'py, PyAny> = native_field(request, "select")?;
    let exclude: Bound<'py, PyAny> = native_field(request, "exclude")?;
    reject_unit_test_selectors(&select.clone(), &exclude.clone(), &discovered.clone())?;
    let selected_target: Bound<'py, PyAny> = native_field(request, "selected_target")?;
    let target_config: Bound<'py, PyAny> = build_effective_target_config(
        &discovered.clone(),
        Some(selected_target.clone()).filter(|value| !value.is_none()),
    )?;
    let defaults: LoaderDefaults<'py> = loader_defaults(
        &discovered,
        (
            &selected_target,
            &target_config,
            &native_field(request, "cli_vars")?,
        ),
    )?;
    let vars: Bound<'py, PyAny> = target_vars(
        &discovered,
        (&selected_target, &native_field(request, "cli_vars")?),
    )?;
    let project: LoadProject<'py> = LoadProject::new(&discovered, (&target_config, &vars))?;
    let selected: Vec<Bound<'py, PyAny>> = project.selected_entries(
        (
            &select.extract::<Vec<String>>()?,
            &exclude.extract::<Vec<String>>()?,
        ),
        &defaults,
    )?;
    loader_write_schemas(
        &discovered,
        (&selected_target, &native_field(request, "cli_vars")?),
        &selected,
    )?;
    let references: Vec<Bound<'py, PyAny>> = project.reference_entries(&selected, &defaults)?;
    let machine_output: bool = native_field(request, "json_output")?.extract()?;
    let use_color: bool = !native_field(request, "no_color")?.extract::<bool>()?
        && !machine_output
        && stdout_supports_color(py)?;
    let stream: Bound<'py, PyAny> =
        py.import("sys")?
            .getattr(if machine_output { "stderr" } else { "stdout" })?;
    Ok(Invocation {
        project_dir,
        discovered,
        selected,
        references,
        relation_sources: project.sources,
        style: CliStyle { use_color },
        stream,
    })
}

fn user_error(py: Python<'_>, message: String) -> PyErr {
    match error_class(py, ErrorId::CliUser).and_then(|class| class.call1((message,))) {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn target_name<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let table: Bound<'_, PyAny> = native_field(source, "table")?;
    if table.is_truthy()? {
        Ok(table)
    } else {
        native_field(source, "name")
    }
}

/// Refuse a load whose skipped intermediate loaders have no target relation yet.
fn check_reference_targets(
    invocation: &Invocation<'_>,
    (adapter, connection_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<()> {
    if invocation.references.is_empty() {
        return Ok(());
    }
    let py: Python<'_> = adapter.py();
    let selected_names: Vec<String> = invocation
        .selected
        .iter()
        .map(|source| native_field(source, "name")?.extract())
        .collect::<PyResult<_>>()?;
    let connection: Bound<'_, PyAny> = adapter
        .call_method1("connect", (connection_config,))
        .map_err(|error| connection_failure(adapter, &error))?;
    let checked: PyResult<()> = (|| {
        let locations: Vec<Bound<'_, PyTuple>> = invocation
            .references
            .iter()
            .map(|source| {
                PyTuple::new(
                    py,
                    [
                        native_field(source, "database")?,
                        native_field(source, "schema")?,
                        target_name(source)?,
                    ],
                )
            })
            .collect::<PyResult<_>>()?;
        let lookup: Bound<'_, PyAny> = call_with(
            &host::entry(py, AdapterHost::BuildRelationLookup)?,
            &[
                ("adapter", adapter.clone()),
                ("connection", connection.clone()),
                ("locations", PyTuple::new(py, locations)?.into_any()),
            ],
        )?;
        for source in &invocation.references {
            let target: Bound<'_, PyAny> = target_name(source)?;
            let exists: bool = call_with(
                &native_field(&lookup, "exists")?,
                &[
                    ("database", native_field(source, "database")?),
                    ("schema", native_field(source, "schema")?),
                    ("name", target.clone()),
                ],
            )?
            .is_truthy()?;
            if !exists {
                return Err(user_error(
                    py,
                    format!(
                        "Selected source load requires intermediate loader '{}', but its target \
                         relation '{}' does not exist; use +{} to refresh upstream ingress \
                         dependencies",
                        native_field(source, "name")?.str()?,
                        target.str()?,
                        selected_names.join(", ")
                    ),
                ));
            }
        }
        Ok(())
    })();
    let closed: PyResult<Bound<'_, PyAny>> = adapter.call_method1("close", (&connection,));
    checked?;
    closed?;
    Ok(())
}

/// Source relations loaders must not hard-code, or `None` when enforcement is off.
fn project_relations<'py>(
    invocation: &Invocation<'py>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = adapter.py();
    if !native_field(&invocation.discovered, "project_config")?
        .getattr("references")?
        .getattr("enforce_explicit")?
        .is_truthy()?
    {
        return Ok(py.None().into_bound(py));
    }
    let sources: Bound<'py, PyDict> = PyDict::new(py);
    for source in invocation
        .relation_sources
        .iter()
        .chain(&invocation.references)
        .chain(&invocation.selected)
    {
        if native_field(source, "expression")?.is_none() {
            sources.set_item(native_field(source, "name")?, source)?;
        }
    }
    let reference_class: Bound<'py, PyAny> = host::entry(py, UserCodeHost::SqlResourceRef)?;
    let source_kind: Bound<'py, PyAny> =
        host::entry(py, UserCodeHost::SqlResourceRefKind)?.getattr("SOURCE")?;
    let relations: Bound<'py, PyDict> = PyDict::new(py);
    for (name, source) in sources.iter() {
        let key: Bound<'py, PyAny> = call_with(
            &reference_class,
            &[("kind", source_kind.clone()), ("name", name)],
        )?;
        let relation: String =
            render_source_relation(&source, Some(adapter).filter(|value| !value.is_none()))?;
        relations.set_item(key, relation)?;
    }
    Ok(relations.into_any())
}

fn prepare<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Invocation<'py>,
) -> PyResult<Preparation<'py>> {
    let py: Python<'py> = request.py();
    let discovered: &Bound<'py, PyAny> = &invocation.discovered;
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
    )?;
    let adapter: Bound<'py, PyAny> = resolve_cli_adapter(&adapter_name, &invocation.project_dir)?;
    let connection_config: Bound<'py, PyAny> = resolve_project_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("project_dir", invocation.project_dir.clone()),
                ("selected_target", native_field(request, "selected_target")?),
                ("cli_vars", native_field(request, "cli_vars")?),
            ],
        )?),
    )?;
    check_reference_targets(invocation, (&adapter, &connection_config))?;
    let runtime: (Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyAny>) =
        build_effective_runtime_config(
            &discovered.clone(),
            Some(native_field(request, "selected_target")?).filter(|value| !value.is_none()),
            Some(native_field(request, "cli_vars")?).filter(|value| !value.is_none()),
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
        .extract()?;
    let concurrency: i64 = resolve_concurrency(
        &native_field(request, "concurrency")?,
        discovered,
        &native_field(request, "selected_target")?,
    )?
    .limit;
    let typed: Bound<'py, PyAny> =
        type_cursor_overrides(&native_field(request, "cursor_overrides")?, &[])?;
    let cursor_overrides: Bound<'py, PyAny> = if typed.is_truthy()? {
        typed
    } else {
        new_record(py, RecordId::CursorOverrides, (), None)?
    };
    let provider_session: Bound<'py, PyAny> = call_with(
        &host::entry(py, UserCodeHost::BuildProviderSession)?,
        &[(
            "discovered_providers",
            native_field(discovered, "providers")?,
        )],
    )?;
    Ok(Preparation {
        adapter_name: adapter_name.extract()?,
        project_relations: project_relations(invocation, &adapter)?,
        adapter,
        connection_config,
        runtime,
        concurrency: concurrency.max(1),
        cursor_overrides,
        provider_session,
    })
}

/// The executor's runtime parameters for this load.
fn runtime_params<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Invocation<'py>,
    preparation: &Preparation<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let (target_name, effective_vars, run_id) = &preparation.runtime;
    let overrides: &Bound<'py, PyAny> = &preparation.cursor_overrides;
    call_with(
        &host::entry(py, ExecutorHost::LoadRuntimeParams)?,
        &[
            ("run_id", run_id.clone()),
            (
                "runtime_dir",
                invocation
                    .project_dir
                    .call_method1("__truediv__", ("target",))?,
            ),
            ("target", target_name.clone()),
            ("vars", effective_vars.clone()),
            ("is_reload", native_field(request, "reload")?),
            (
                "start_cursor_ts",
                cursor_timestamp(&native_field(overrides, "start_ts")?)?,
            ),
            (
                "end_cursor_ts",
                cursor_timestamp(&native_field(overrides, "end_ts")?)?,
            ),
            (
                "start_cursor_int",
                cursor_integer(&native_field(overrides, "start_int")?)?,
            ),
            (
                "end_cursor_int",
                cursor_integer(&native_field(overrides, "end_int")?)?,
            ),
            (
                "use_color",
                PyBool::new(py, invocation.style.use_color)
                    .to_owned()
                    .into_any(),
            ),
            (
                "providers",
                native_field(&preparation.provider_session, "providers")?,
            ),
            ("project_relations", preparation.project_relations.clone()),
        ],
    )
}

/// Run the selected loads with progress callbacks; returns the results and elapsed seconds.
fn execute<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Invocation<'py>,
    preparation: &Preparation<'py>,
) -> PyResult<(Vec<Bound<'py, PyAny>>, f64)> {
    let py: Python<'py> = request.py();
    let started: Instant = Instant::now();
    let event_writer: Bound<'py, PyAny> = ExecutionEventWriter::new(py, None)
        .and_then(|value| pyo3::Bound::new(py, value))
        .map(pyo3::Bound::into_any)?;
    let order: Bound<'py, PyDict> = PyDict::new(py);
    for (index, source) in invocation.selected.iter().enumerate() {
        order.set_item(native_field(source, "name")?, index + 1)?;
    }
    let reporter: Bound<'py, LoadProgressReporter> =
        crate::bindings::_helpers::load::load_progress::LoadProgressReporter::new(
            py,
            Some(
                {
                    let options: Bound<'py, PyDict> = PyDict::new(py);
                    options.set_item("stream", &invocation.stream)?;
                    options.set_item("use_color", invocation.style.use_color)?;
                    options.set_item("source_order", order)?;
                    options.set_item("total_count", invocation.selected.len())?;
                    options
                }
                .clone(),
            ),
        )
        .and_then(|value| pyo3::Bound::new(py, value))
        .map(pyo3::Bound::into_any)?
        .downcast_into()?;
    let connection: Bound<'py, PyAny> = ConnectionProgressReporter::new(
        py,
        Some(keywords(
            py,
            &[
                ("adapter_name", text(py, &preparation.adapter_name)),
                ("stream", invocation.stream.clone()),
                (
                    "blank_line_after_complete",
                    PyBool::new(py, true).to_owned().into_any(),
                ),
                (
                    "use_color",
                    PyBool::new(py, invocation.style.use_color)
                        .to_owned()
                        .into_any(),
                ),
            ],
        )?),
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let relay: Bound<'py, LoadCallbackRelay> = Bound::new(
        py,
        LoadCallbackRelay {
            reporter: reporter.clone().unbind(),
            event_writer: event_writer.clone().unbind(),
            connection: connection.clone().unbind(),
        },
    )?;
    let executed: PyResult<Vec<Bound<'py, PyAny>>> = (|| {
        let callbacks: Bound<'py, PyAny> = call_with(
            &host::entry(py, ExecutorHost::LoadCallbacks)?,
            &[
                ("on_load_start", native_field(&reporter, "on_start")?),
                ("on_load_progress", native_field(&reporter, "on_progress")?),
                (
                    "on_load_complete",
                    native_field(&relay, "on_load_complete")?,
                ),
                (
                    "on_connection_start",
                    native_field(&connection, "on_connection_start")?,
                ),
                (
                    "on_connection_complete",
                    native_field(&relay, "on_connection_complete")?,
                ),
                (
                    "on_connection_error",
                    native_field(&relay, "on_connection_error")?,
                ),
            ],
        )?;
        call_with(
            &host::entry(py, ExecutorHost::RunLoadPipeline)?,
            &[
                (
                    "sources",
                    PyTuple::new(py, &invocation.selected)?.into_any(),
                ),
                (
                    "reference_sources",
                    PyTuple::new(py, &invocation.references)?.into_any(),
                ),
                (
                    "loader_functions",
                    native_field(&invocation.discovered, "loader_functions")?,
                ),
                ("connection_config", preparation.connection_config.clone()),
                ("adapter", preparation.adapter.clone()),
                (
                    "max_concurrency",
                    PyInt::new(py, preparation.concurrency).into_any(),
                ),
                ("runtime", runtime_params(request, invocation, preparation)?),
                ("callbacks", callbacks),
            ],
        )?
        .try_iter()?
        .collect()
    })();
    let closed: PyResult<Bound<'py, PyAny>> = event_writer.call_method0("close");
    let results: Vec<Bound<'py, PyAny>> = executed?;
    closed?;
    Ok((results, started.elapsed().as_secs_f64()))
}

fn write_json(request: &Bound<'_, PyAny>, results: &[Bound<'_, PyAny>]) -> PyResult<()> {
    let py: Python<'_> = request.py();
    let payload: Bound<'_, PyAny> =
        format_load_execution_json(py, PyTuple::new(py, results)?.into_any())
            .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    write_execution_json_output(
        py,
        payload,
        native_field(request, "json_output")?.extract::<bool>()?,
        native_field(request, "json_output_path")?,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    Ok(())
}

fn loader_node_flag(source: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py: Python<'_> = source.py();
    Ok(native_field(source, "meta")?
        .call_method1("get", (LOADER_NODE_KEY,))?
        .is(PyBool::new(py, true).to_owned()))
}

/// Run one `sqb load` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_load_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let invocation: Invocation<'_> = invocation(py, request)?;
    write(
        &invocation.stream,
        &render_load_ready(invocation.selected.len(), invocation.style),
    )?;
    if invocation.selected.is_empty() {
        write(&invocation.stream, &render_empty_load(invocation.style))?;
        write_json(request, &[])?;
        return Ok(0);
    }
    let preparation: Preparation<'_> = prepare(request, &invocation)?;
    let outcome: PyResult<(Vec<Bound<'_, PyAny>>, f64)> = (|| {
        let entries: Vec<(String, bool)> = invocation
            .selected
            .iter()
            .map(|source| {
                Ok((
                    native_field(source, "name")?.extract()?,
                    loader_node_flag(source)?,
                ))
            })
            .collect::<PyResult<_>>()?;
        write(
            &invocation.stream,
            &render_load_plan(&entries, invocation.style),
        )?;
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("stream", &invocation.stream)?;
        options.set_item("command", EXECUTION_COMMAND)?;
        options.set_item("target", py.None())?;
        options.set_item("concurrency", preparation.concurrency)?;
        options.set_item("use_color", invocation.style.use_color)?;
        write_execution_header(py, Some(options.clone()))
            .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
        execute(request, &invocation, &preparation)
    })();
    let closed: PyResult<Bound<'_, PyAny>> = preparation.provider_session.call_method0("close");
    let (results, elapsed) = outcome?;
    closed?;
    let texts: Vec<LoadResultText> = results
        .iter()
        .map(load_result_text)
        .collect::<PyResult<_>>()?;
    let counts: LoadCounts = load_counts(&texts);
    write(
        &invocation.stream,
        &render_load_footer(&texts, (counts, elapsed), invocation.style),
    )?;
    write_json(request, &results)?;
    Ok(load_exit_code(counts))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_load_command, module)?)?;
    Ok(())
}
