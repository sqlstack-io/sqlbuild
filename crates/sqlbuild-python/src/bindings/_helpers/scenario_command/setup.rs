//! Shared `sqb scenario` preparation: project, namespace, adapters, compile and selection.

use sqlbuild_python_base::records::main::construct::new_error::new_error;
use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyInt, PyString, PyTuple};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::scenario_command::constants::SCENARIO_NAMESPACE_ENV;
use sqlbuild_core::scenario_command::errors::ScenarioCommandError;
use sqlbuild_core::scenario_command::main::capture_dialect_required_error::capture_dialect_required_error;
use sqlbuild_core::scenario_command::main::format_captured_at::format_captured_at;
use sqlbuild_core::scenario_command::main::render_namespace_header::render_namespace_header;
use sqlbuild_core::scenario_command::main::resolve_scenario_namespace::resolve_scenario_namespace;
use sqlbuild_core::scenario_command::main::resolve_snapshot_limits::resolve_snapshot_limits;
use sqlbuild_core::scenario_command::main::select_scenarios::select_scenarios;
use sqlbuild_core::scenario_command::main::sql_analysis_purpose::sql_analysis_purpose;
use sqlbuild_core::scenario_command::main::sql_analysis_required_error::sql_analysis_required_error;
use sqlbuild_core::scenario_command::models::{
    NamespaceCandidates, ScenarioFile, ScenarioNamespace, ScenarioSelection, SnapshotLimitConfig,
    SnapshotLimitInputs, SnapshotLimits, SqlAnalysisCommand,
};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::command_runtime::connection::resolve_project_connection_config;
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::command_runtime::main::resolve_cli_adapter::resolve_cli_adapter;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::presentation_text::main::stdout_supports_color::stdout_supports_color;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_compile::compile_driver::main::setup::run_compile_only_pipeline::run_compile_only_pipeline;
use sqlbuild_python_compile::compile_driver::main::stages::discover_project_inputs::discover_project_inputs;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_front::contracts::main::resolve_effective_scenario_config::resolve_effective_scenario_config;
use sqlbuild_python_front::discovery::main::failures::sql_analysis_guidance::sql_analysis_guidance;
use sqlbuild_python_output::build_progress::main::write_execution_header::write_execution_header;
use sqlbuild_python_output::build_progress::models::PlanningProgressReporter;
use sqlbuild_python_project::planner_selection::main::reject_unit_test_selectors::reject_unit_test_selectors;

const METADATA_MODULE: &str = "importlib.metadata";
const DISTRIBUTION_NAME: &str = "sqlbuild";
const UNKNOWN_VERSION: &str = "unknown";

pub(crate) fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

pub(crate) fn optional_text<'py>(py: Python<'py>, value: Option<&str>) -> Bound<'py, PyAny> {
    value.map_or_else(|| py.None().into_bound(py), |value| text(py, value))
}

/// `CliUserError(message, code=..., help=...)`.
pub(crate) fn cli_user_error(py: Python<'_>, error: ScenarioCommandError) -> PyErr {
    new_error(
        py,
        ErrorId::CliUser,
        &error.message,
        (Some(error.code), error.help.as_deref()),
    )
}

/// The progress stream: `write` then `flush`.
pub(crate) fn write(stream: &Bound<'_, PyAny>, value: &str) -> PyResult<()> {
    stream.call_method1("write", (value,))?;
    Ok(())
}

pub(crate) fn flush(stream: &Bound<'_, PyAny>) -> PyResult<()> {
    stream.call_method0("flush")?;
    Ok(())
}

/// `hasattr(stream, "isatty") and stream.isatty()`.
pub(crate) fn stream_isatty(stream: &Bound<'_, PyAny>) -> PyResult<bool> {
    if !stream.hasattr("isatty")? {
        return Ok(false);
    }
    stream.call_method0("isatty")?.is_truthy()
}

/// The project, its discovered inputs and the effective scenario namespace.
#[derive(Clone)]
pub(crate) struct ScenarioProject<'py> {
    pub(crate) project_dir: Bound<'py, PyAny>,
    pub(crate) discovered: Bound<'py, PyAny>,
    pub(crate) namespace: ScenarioNamespace,
    pub(crate) selectors: Bound<'py, PyAny>,
    pub(crate) exclude: Bound<'py, PyAny>,
}

fn config_namespace(config: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    native_field(&native_field(config, "scenario")?, "run_namespace")?.extract()
}

/// `dataclasses.replace(value, **{name: replacement})`.
fn replaced<'py>(
    value: &Bound<'py, PyAny>,
    (name, replacement): (&str, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let changes: Bound<'py, PyDict> = PyDict::new(value.py());
    changes.set_item(name, replacement)?;
    replaced_keywords(value, &changes)
}

/// Discover the project, reject unit-test selectors and apply the effective namespace.
pub(crate) fn discover_project<'py>(
    py: Python<'py>,
    request: &Bound<'py, PyAny>,
) -> PyResult<ScenarioProject<'py>> {
    let project_dir: Bound<'py, PyAny> = match native_field(request, "project_dir")? {
        directory if !directory.is_none() => directory,
        _ => py.import("pathlib")?.getattr("Path")?.call_method0("cwd")?,
    };
    let discovered: Bound<'py, PyAny> = discover_project_inputs(
        py,
        Some(keywords(py, &[("project_dir", project_dir.clone())])?),
    )?;
    let selectors: Bound<'py, PyAny> = native_field(request, "selectors")?;
    let exclude: Bound<'py, PyAny> = native_field(request, "exclude")?;
    reject_unit_test_selectors(&selectors.clone(), &exclude.clone(), &discovered.clone())?;
    let local_config: Bound<'py, PyAny> = native_field(&discovered, "local_config")?;
    let candidates: NamespaceCandidates = NamespaceCandidates {
        cli: native_field(request, "scenario_namespace")?.extract()?,
        env: py
            .import("os")?
            .getattr("environ")?
            .call_method1("get", (SCENARIO_NAMESPACE_ENV,))?
            .extract()?,
        local_config: config_namespace(&local_config)?,
        project_config: config_namespace(&native_field(&discovered, "project_config")?)?,
    };
    let namespace: ScenarioNamespace =
        resolve_scenario_namespace(candidates).map_err(|error| cli_user_error(py, error))?;
    let discovered: Bound<'py, PyAny> = match &namespace.value {
        Some(value) => {
            let scenario: Bound<'py, PyAny> = replaced(
                &native_field(&local_config, "scenario")?,
                ("run_namespace", text(py, value)),
            )?;
            replaced(
                &discovered,
                (
                    "local_config",
                    replaced(&local_config, ("scenario", scenario))?,
                ),
            )?
        }
        None => discovered,
    };
    Ok(ScenarioProject {
        project_dir,
        discovered,
        namespace,
        selectors,
        exclude,
    })
}

/// Raise when SQL analysis is off by flag or by a config file.
pub(crate) fn require_sql_analysis(
    project: &ScenarioProject<'_>,
    (command, disabled_by_flag): (SqlAnalysisCommand, bool),
) -> PyResult<()> {
    let py: Python<'_> = project.discovered.py();
    let guidance: Option<(String, String)> = if disabled_by_flag {
        None
    } else {
        sql_analysis_guidance(&project.discovered, sql_analysis_purpose(command))?
    };
    match sql_analysis_required_error(command, disabled_by_flag, guidance) {
        Some(error) => Err(cli_user_error(py, error)),
        None => Ok(()),
    }
}

/// The adapter named by the project's effective configuration.
pub(crate) fn project_adapter_name(project: &ScenarioProject<'_>) -> PyResult<String> {
    resolve_effective_adapter_name(
        &native_field(&project.discovered, "project_config")?,
        &native_field(&project.discovered, "local_config")?,
    )?
    .extract()
}

pub(crate) fn resolve_adapter<'py>(
    project: &ScenarioProject<'py>,
    adapter_name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.discovered.py();
    resolve_cli_adapter(&text(py, adapter_name), &project.project_dir)
}

/// The adapter's SQL analysis dialect, which snapshots require.
pub(crate) fn require_capture_dialect(
    adapter: &Bound<'_, PyAny>,
    adapter_name: &str,
) -> PyResult<String> {
    let dialect: Option<String> = adapter.call_method0("sql_analysis_dialect")?.extract()?;
    dialect
        .ok_or_else(|| cli_user_error(adapter.py(), capture_dialect_required_error(adapter_name)))
}

pub(crate) fn project_connection_config<'py>(
    project: &ScenarioProject<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    connection_config(project, true)
}

/// The project's connection settings for an offline compile, which needs no warehouse connection.
pub(crate) fn offline_project_connection_config<'py>(
    project: &ScenarioProject<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    connection_config(project, false)
}

fn connection_config<'py>(
    project: &ScenarioProject<'py>,
    require_connection: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.discovered.py();
    resolve_project_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", project.discovered.clone()),
                ("project_dir", project.project_dir.clone()),
                ("require_connection", boolean(py, require_connection)),
            ],
        )?),
    )
}

/// `not no_color and supports_color()`.
pub(crate) fn style(request: &Bound<'_, PyAny>) -> PyResult<CliStyle> {
    let use_color: bool =
        !native_field(request, "no_color")?.is_truthy()? && stdout_supports_color(request.py())?;
    Ok(CliStyle { use_color })
}

/// The planning reporter a scenario compile reports its phases to.
pub(crate) fn planning_reporter<'py>(
    stream: &Bound<'py, PyAny>,
    style: CliStyle,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = stream.py();
    Ok(Bound::new(
        py,
        PlanningProgressReporter::new(py, stream, style.use_color)?,
    )?
    .into_any())
}

/// `Execution  sqb scenario test  (concurrency: N, DuckDB)`, after the run's header.
pub(crate) fn write_execution<'py>(
    stream: &Bound<'py, PyAny>,
    project: &ScenarioProject<'py>,
    (command, (concurrency, engines), style): (&str, (i64, Option<String>), CliStyle),
) -> PyResult<()> {
    let py: Python<'py> = stream.py();
    let selectors: Vec<String> = project.selectors.extract()?;
    write(stream, "\n")?;
    let target: Option<String> = (!selectors.is_empty()).then(|| selectors.join(" "));
    write_execution_header(
        py,
        Some(keywords(
            py,
            &[
                ("stream", stream.clone()),
                ("command", text(py, command)),
                ("target", optional_text(py, target.as_deref())),
                ("concurrency", int(py, concurrency)),
                ("use_color", boolean(py, style.use_color)),
                ("engines", optional_text(py, engines.as_deref())),
            ],
        )?),
    )
}

/// Write the namespace line, then the execution header with the selected target.
pub(crate) fn write_headers<'py>(
    stream: &Bound<'py, PyAny>,
    project: &ScenarioProject<'py>,
    (command, concurrency, style): (&str, i64, CliStyle),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = stream.py();
    write(stream, &render_namespace_header(&project.namespace))?;
    let selectors: Vec<String> = project.selectors.extract()?;
    let planning: Bound<'py, PyAny> = Bound::new(
        py,
        PlanningProgressReporter::new(py, stream, style.use_color)?,
    )?
    .into_any();
    write(stream, "\n")?;
    let target: Option<String> = (!selectors.is_empty()).then(|| selectors.join(" "));
    write_execution_header(
        py,
        Some(keywords(
            py,
            &[
                ("stream", stream.clone()),
                ("command", text(py, command)),
                ("target", optional_text(py, target.as_deref())),
                ("concurrency", int(py, concurrency)),
                ("use_color", boolean(py, style.use_color)),
            ],
        )?),
    )?;
    Ok(planning)
}

/// The adapter, compile options and planning progress of one scenario compile.
pub(crate) struct CompileInputs<'a, 'py> {
    pub(crate) adapter: &'a Bound<'py, PyAny>,
    pub(crate) options: &'a [(&'a str, Bound<'py, PyAny>)],
    pub(crate) planning: &'a Bound<'py, PyAny>,
}

/// Compile the project inside the project compile lifecycle.
pub(crate) fn compile<'py>(
    project: &ScenarioProject<'py>,
    inputs: CompileInputs<'_, 'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let CompileInputs {
        adapter,
        options,
        planning,
    } = inputs;
    let py: Python<'py> = project.discovered.py();
    let resolver: Bound<'py, PyAny> = resolve_external_sql_reference_resolver(
        project.project_dir.clone(),
        project.discovered.clone(),
    )?;
    let mut pairs: Vec<(&str, Bound<'py, PyAny>)> = options.to_vec();
    pairs.push(("external_sql_reference_resolver", resolver));
    let options: Bound<'py, PyAny> = new_record_with(py, RecordId::CompilePipelineOptions, &pairs)?;
    let lifecycle: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::OperationLifecycle)?,
        &[
            ("operation_kind", text(py, "project")),
            ("operation_name", text(py, "project_compile")),
        ],
    )?;
    within(&lifecycle, |_| {
        run_compile_only_pipeline(
            py,
            Some(keywords(
                py,
                &[
                    ("discovered_inputs", project.discovered.clone()),
                    ("adapter", adapter.clone()),
                    ("options", options.clone()),
                    ("on_progress", native_field(planning, "on_progress")?),
                ],
            )?),
        )
    })
}

/// The compiled scenarios the selectors and exclusions select, in selection order.
pub(crate) fn selected_scenarios<'py>(
    pipeline: &Bound<'py, PyAny>,
    (selectors, exclude, project_dir): (&[String], &[String], &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = pipeline.py();
    let compiled: Vec<Bound<'py, PyAny>> =
        native_field(&native_field(pipeline, "project")?, "sql_scenarios")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
    let mut files: Vec<ScenarioFile> = Vec::with_capacity(compiled.len());
    for scenario in &compiled {
        let file: Bound<'py, PyAny> = native_field(scenario, "scenario_file")?;
        files.push(ScenarioFile {
            name: native_field(scenario, "name")?.extract()?,
            file_path: native_field(&file, "file_path")?.extract()?,
            relative_path: native_field(&file, "relative_path")?.extract()?,
        });
    }
    let project_dir: PathBuf = project_dir.extract()?;
    let indices: Vec<usize> = select_scenarios(
        &files,
        ScenarioSelection {
            selectors,
            exclude,
            project_dir: &project_dir,
        },
    )
    .map_err(|error| cli_user_error(py, error))?;
    PyTuple::new(py, indices.into_iter().map(|index| compiled[index].clone()))
}

fn optional_int(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<i64>> {
    native_field(value, name)?.extract()
}

/// The snapshot capture limits from the request, the scenario config and the defaults.
pub(crate) fn capture_limits<'py>(
    request: &Bound<'py, PyAny>,
    project: &ScenarioProject<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let inputs: Bound<'py, PyAny> = native_field(request, "limit_inputs")?;
    let configured: Bound<'py, PyAny> = resolve_effective_scenario_config(
        &native_field(&project.discovered, "project_config")?,
        &native_field(&project.discovered, "local_config")?,
    )?
    .getattr("snapshot_limits")?;
    let limits: SnapshotLimits = resolve_snapshot_limits(
        SnapshotLimitInputs {
            max_rows_per_relation: optional_int(&inputs, "max_snapshot_rows")?,
            max_total_rows: optional_int(&inputs, "max_snapshot_total_rows")?,
            max_bytes_per_relation: optional_int(&inputs, "max_snapshot_bytes")?,
            max_total_bytes: optional_int(&inputs, "max_snapshot_total_bytes")?,
            force: native_field(&inputs, "force")?.is_truthy()?,
        },
        SnapshotLimitConfig {
            max_rows_per_relation: optional_int(&configured, "max_rows_per_relation")?,
            max_total_rows: optional_int(&configured, "max_total_rows")?,
            max_bytes_per_relation: optional_int(&configured, "max_bytes_per_relation")?,
            max_total_bytes: optional_int(&configured, "max_total_bytes")?,
        },
    );
    call_with(
        &host_entry(py, ExecutorHost::ScenarioSnapshotCaptureLimits)?,
        &[
            (
                "max_rows_per_relation",
                int(py, limits.max_rows_per_relation),
            ),
            ("max_total_rows", int(py, limits.max_total_rows)),
            (
                "max_bytes_per_relation",
                int(py, limits.max_bytes_per_relation),
            ),
            ("max_total_bytes", int(py, limits.max_total_bytes)),
            ("force", boolean(py, limits.force)),
        ],
    )
}

fn sqlbuild_version(py: Python<'_>) -> PyResult<String> {
    let metadata: Bound<'_, PyAny> = py.import(METADATA_MODULE)?.into_any();
    match native_field(&metadata, "version")?.call1((DISTRIBUTION_NAME,)) {
        Ok(version) => version.extract(),
        Err(error) if error.is_instance(py, &native_field(&metadata, "PackageNotFoundError")?) => {
            Ok(UNKNOWN_VERSION.to_owned())
        }
        Err(error) => Err(error),
    }
}

/// Capture settings with the current capture time and SQLBuild version.
pub(crate) fn capture_settings<'py>(
    py: Python<'py>,
    (capture_adapter, capture_dialect, retain): (&str, &str, bool),
    limits: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let epoch_seconds: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        });
    call_with(
        &host_entry(py, ExecutorHost::ScenarioCaptureSettings)?,
        &[
            ("captured_at", text(py, &format_captured_at(epoch_seconds))),
            ("capture_adapter", text(py, capture_adapter)),
            ("capture_dialect", text(py, capture_dialect)),
            ("sqlbuild_version", text(py, &sqlbuild_version(py)?)),
            ("retain", boolean(py, retain)),
            ("limits", limits),
        ],
    )
}

pub(crate) fn int(py: Python<'_>, value: i64) -> Bound<'_, PyAny> {
    PyInt::new(py, value).into_any()
}

pub(crate) fn boolean(py: Python<'_>, value: bool) -> Bound<'_, PyAny> {
    PyBool::new(py, value).to_owned().into_any()
}
