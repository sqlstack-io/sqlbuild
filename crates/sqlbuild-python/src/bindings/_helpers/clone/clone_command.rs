//! `sqb clone`: copy or clone selected relations from an origin target into a destination target.

use std::collections::BTreeSet;
use std::time::Instant;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::main::format_phase_line::format_phase_line;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::clone_text::errors::CloneTargetError;
use sqlbuild_core::clone_text::main::clone_exit_code::clone_exit_code;
use sqlbuild_core::clone_text::main::render_clone_completion::render_clone_completion;
use sqlbuild_core::clone_text::main::render_clone_header::render_clone_header;
use sqlbuild_core::clone_text::main::validate_clone_policy::validate_clone_policy;
use sqlbuild_core::clone_text::main::validate_clone_target_names::validate_clone_target_names;
use sqlbuild_core::clone_text::models::CloneItemText;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::build_progress::progress_reporters::PlanningProgressReporter;
use crate::bindings::_helpers::clone::clone_execution::{
    destination_entry_count, execute_clone_plan, resource_types_by_name,
};
use crate::bindings::_helpers::clone::clone_pipeline::{ClonePipeline, timed_clone_pipeline};
use crate::bindings::_helpers::clone::clone_progress::{clone_item_text, write_line};
use crate::bindings::_helpers::command_runtime::connection::resolve_target_connection_config;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_name;
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use crate::bindings::_helpers::planner_entries::clone_plans::PhaseTimings;
use crate::bindings::_helpers::planner_selection::unit_tests::reject_unit_test_selectors;
use crate::bindings::_helpers::result_output::documents::format_clone_execution_json;
use crate::bindings::_helpers::result_output::documents::write_execution_json_output;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::command_runtime::main::resolve_cli_adapter::resolve_cli_adapter;
use sqlbuild_python_base::native_host::main::connection_failure::connection_failure;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::presentation_text::main::stdout_supports_color::stdout_supports_color;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const CLONE_LOGGER: &str = "sqlbuild.clone";
/// The clone phase that copies relations into the destination.
const EXECUTION_PHASE: &str = "execution";
const NO_DESTINATION: (&str, &str) = (
    "C406",
    "clone requires --to when the project has no active target",
);
const NOTHING_TO_CLONE: (&str, &str) =
    ("C407", "no cloneable resources found in the selected scope");

/// The discovered project, adapter and output context of one clone.
pub(crate) struct Invocation<'py> {
    pub(crate) project_dir: Bound<'py, PyAny>,
    pub(crate) discovered: Bound<'py, PyAny>,
    pub(crate) adapter_name: String,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) destination: String,
    pub(crate) style: CliStyle,
    /// The stdout of the invocation, which progress keeps writing to while output is captured.
    pub(crate) stream: Bound<'py, PyAny>,
}

fn user_error(py: Python<'_>, (code, message): (&str, &str)) -> PyErr {
    let raised = (|| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("code", code)?;
        error_class(py, ErrorId::CliUser)?.call((message,), Some(&options))
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn target_error(py: Python<'_>, error: CloneTargetError) -> PyErr {
    user_error(py, (error.code, &error.message))
}

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn progress_reporter<'py>(
    stream: &Bound<'py, PyAny>,
    style: CliStyle,
) -> PyResult<Bound<'py, PlanningProgressReporter>> {
    let py: Python<'py> = stream.py();
    Bound::new(
        py,
        PlanningProgressReporter::new(py, stream, style.use_color)?,
    )
}

fn lifecycle<'py>(py: Python<'py>, (kind, name): (&str, &str)) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host_entry(py, ExecutorHost::OperationLifecycle)?,
        &[
            ("operation_kind", text(py, kind)),
            ("operation_name", text(py, name)),
        ],
    )
}

fn target_names(discovered: &Bound<'_, PyAny>) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for config in ["project_config", "local_config"] {
        for name in native_field(&native_field(discovered, config)?, "targets")?.try_iter()? {
            names.insert(name?.extract()?);
        }
    }
    Ok(names)
}

fn clone_policy(discovered: &Bound<'_, PyAny>, (target, flag): (&str, &str)) -> PyResult<bool> {
    let py: Python<'_> = discovered.py();
    let config = resolve_target_config(
        py,
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        &text(py, target),
    )?;
    native_field(&native_field(&config, "clone")?, flag)?.is_truthy()
}

/// Discover the project, resolve and validate the targets, and resolve the adapter.
fn invocation<'py>(py: Python<'py>, request: &Bound<'py, PyAny>) -> PyResult<Invocation<'py>> {
    let project_dir: Bound<'py, PyAny> = match native_field(request, "project_dir")? {
        directory if !directory.is_none() => directory,
        _ => py.import("pathlib")?.getattr("Path")?.call_method0("cwd")?,
    };
    let use_color: bool =
        !native_field(request, "no_color")?.extract::<bool>()? && stdout_supports_color(py)?;
    let style: CliStyle = CliStyle { use_color };
    let stream: Bound<'py, PyAny> = py.import("sys")?.getattr("stdout")?;
    let progress: Bound<'py, PlanningProgressReporter> = progress_reporter(&stream, style)?;
    progress.get().start(py, "Discovering project...")?;
    let supplied: Bound<'py, PyAny> = native_field(request, "discovered_inputs")?;
    let discovered: Bound<'py, PyAny> = if supplied.is_none() {
        discover_project_inputs(
            py,
            Some(keywords(py, &[("project_dir", project_dir.clone())])?),
        )?
    } else {
        supplied
    };
    progress.get().complete(py, "Discovered project.")?;
    reject_unit_test_selectors(
        &native_field(request, "select")?,
        &native_field(request, "exclude")?,
        &discovered.clone(),
    )?;
    let mut destination: Bound<'py, PyAny> = native_field(request, "destination_target_name")?;
    if destination.is_none() {
        destination = resolve_target_name(
            py,
            &native_field(&discovered, "project_config")?,
            &native_field(&discovered, "local_config")?,
            &py.None().into_bound(py),
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    }
    if destination.is_none() {
        return Err(user_error(py, NO_DESTINATION));
    }
    let destination: String = destination.extract()?;
    let origin: String = native_field(request, "origin_target_name")?.extract()?;
    let targets: (&str, &str) = (&origin, &destination);
    validate_clone_target_names(targets, &target_names(&discovered)?)
        .map_err(|error| target_error(py, error))?;
    let policy: (bool, bool) = (
        clone_policy(&discovered, (&origin, "allow_as_clone_origin"))?,
        clone_policy(&discovered, (&destination, "allow_as_clone_destination"))?,
    );
    validate_clone_policy(targets, policy).map_err(|error| target_error(py, error))?;
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    let adapter: Bound<'py, PyAny> = resolve_cli_adapter(&adapter_name, &project_dir)?;
    Ok(Invocation {
        project_dir,
        discovered,
        adapter_name: adapter_name.extract()?,
        adapter,
        destination,
        style,
        stream,
    })
}

/// Resolve the destination connection config and connect, returning both.
fn connect<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Invocation<'py>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = request.py();
    let config: Bound<'py, PyAny> = resolve_target_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", invocation.discovered.clone()),
                ("project_dir", invocation.project_dir.clone()),
                ("target_name", text(py, &invocation.destination)),
                ("cli_vars", native_field(request, "cli_vars")?),
            ],
        )?),
    )?;
    let progress: Bound<'py, PlanningProgressReporter> =
        progress_reporter(&invocation.stream, invocation.style)?;
    progress
        .get()
        .start(py, &format!("Connecting to {}...", invocation.adapter_name))?;
    let started: Instant = Instant::now();
    let connection: Bound<'py, PyAny> = within(
        &lifecycle(py, ("clone", "clone_target_connection"))?,
        |_| invocation.adapter.call_method1("connect", (&config,)),
    )
    .map_err(|error| connection_failure(&invocation.adapter, &error))?;
    let summary: String = format!(
        "{}  ({:.2}s)",
        invocation.adapter_name,
        started.elapsed().as_secs_f64()
    );
    progress.get().complete_styled(
        py,
        &format_phase_line(invocation.style, true, ("Warehouse connected", &summary)),
    )?;
    Ok((config, connection))
}

/// Run the clone pipeline and refuse an empty selection.
fn prepare<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Invocation<'py>,
    (config, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<(Bound<'py, PyAny>, PhaseTimings)> {
    let py: Python<'py> = request.py();
    let progress: Bound<'py, PlanningProgressReporter> =
        progress_reporter(&invocation.stream, invocation.style)?;
    let started: Instant = Instant::now();
    let prepared: PyResult<(Bound<'py, PyAny>, PhaseTimings)> =
        within(&lifecycle(py, ("project", "project_compile"))?, |_| {
            progress.get().on_progress(py, "Preparing clone plan...")?;
            let resolver: Bound<'py, PyAny> = resolve_external_sql_reference_resolver(
                invocation.project_dir.clone(),
                invocation.discovered.clone(),
            )?;
            let destination_connection: Bound<'py, PyAny> = new_record_with(
                py,
                RecordId::ClonePipelineConnection,
                &[("config", config.clone()), ("handle", connection.clone())],
            )?;
            let options: Bound<'py, PyAny> = new_record_with(
                py,
                RecordId::ClonePipelineOptions,
                &[
                    (
                        "no_sql_validation",
                        native_field(request, "no_sql_validation")?,
                    ),
                    ("select", native_field(request, "select")?),
                    ("exclude", native_field(request, "exclude")?),
                    ("cli_vars", native_field(request, "cli_vars")?),
                ],
            )?;
            let prepared: (Bound<'py, PyAny>, PhaseTimings) = timed_clone_pipeline(
                &ClonePipeline {
                    discovered: invocation.discovered.clone(),
                    adapter: invocation.adapter.clone(),
                    connection: destination_connection,
                    options,
                    resolver,
                },
                (
                    &native_field(request, "origin_target_name")?,
                    &text(py, &invocation.destination),
                ),
            )?;
            progress.get().on_progress(
                py,
                &format!(
                    "Prepared clone plan. ({:.2}s)",
                    started.elapsed().as_secs_f64()
                ),
            )?;
            Ok(prepared)
        });
    let (pipeline, timings) =
        prepared.or_else(|error| progress.get().finish(py, false).and(Err(error)))?;
    if destination_entry_count(&pipeline)? == 0 {
        return Err(user_error(py, NOTHING_TO_CLONE));
    }
    Ok((pipeline, timings))
}

/// Prepare, announce and execute the clone with an open destination connection.
fn clone<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Invocation<'py>,
    (config, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>, PhaseTimings)> {
    let (pipeline, mut timings) = prepare(request, invocation, (config, connection))?;
    let origin: String = native_field(request, "origin_target_name")?.extract()?;
    write_line(
        &invocation.stream,
        &render_clone_header(
            (&origin, &invocation.destination),
            destination_entry_count(&pipeline)?,
            invocation.style,
        ),
    )?;
    let (result, elapsed) = execute_clone_plan(request, invocation, (&pipeline, connection))?;
    timings.push((EXECUTION_PHASE.to_owned(), elapsed));
    Ok((pipeline, result, timings))
}

/// Log each clone phase's duration for `--debug`, the way compile phases are reported.
fn log_phase_timings(py: Python<'_>, timings: &[(String, f64)]) -> PyResult<()> {
    let logger: Bound<'_, PyAny> = imported(py, "logging", "getLogger")?.call1((CLONE_LOGGER,))?;
    for (phase, seconds) in timings {
        logger.call_method1("debug", (format!("Clone timing  {phase}  {seconds:.3}s"),))?;
    }
    Ok(())
}

/// Run one `sqb clone` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_clone_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let invocation: Invocation<'_> = invocation(py, request)?;
    let (config, connection) = connect(request, &invocation)?;
    let outcome = clone(request, &invocation, (&config, &connection));
    let closed: PyResult<Bound<'_, PyAny>> =
        invocation.adapter.call_method1("close", (&connection,));
    let (pipeline, result, timings) = outcome?;
    closed?;
    log_phase_timings(py, &timings)?;
    let elapsed: f64 = timings
        .iter()
        .find(|(phase, _)| phase == EXECUTION_PHASE)
        .map_or(0.0, |(_, seconds)| *seconds);
    let items: Vec<CloneItemText> = native_field(&result, "item_results")?
        .try_iter()?
        .map(|item| clone_item_text(&item?))
        .collect::<PyResult<_>>()?;
    for line in render_clone_completion(&items, Some(elapsed), invocation.style).lines() {
        write_line(&invocation.stream, line)?;
    }
    let phase_timings: Bound<'_, PyDict> = PyDict::new(py);
    for (phase, seconds) in &timings {
        phase_timings.set_item(phase, seconds)?;
    }
    let resource_types: Bound<'_, PyDict> = PyDict::new(py);
    for (name, resource_type) in resource_types_by_name(&pipeline)? {
        resource_types.set_item(name, resource_type)?;
    }
    let payload: Bound<'_, PyAny> = format_clone_execution_json(
        py,
        result.clone(),
        resource_types.into_any(),
        Some(phase_timings),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    write_execution_json_output(
        py,
        payload,
        PyBool::new(py, false)
            .to_owned()
            .into_any()
            .extract::<bool>()?,
        native_field(request, "json_output_path")?,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    Ok(clone_exit_code(&items))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_clone_command, module)?)?;
    Ok(())
}
