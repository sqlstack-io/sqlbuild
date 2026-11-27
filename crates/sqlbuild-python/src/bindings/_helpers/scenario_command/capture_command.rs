//! `sqb scenario capture`: capture the selected scenarios' inputs to durable local snapshots.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyTuple, PyTupleMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::main::format_summary_footer::format_summary_footer;
use sqlbuild_core::cli_text::models::{CliStyle, SummaryCount};
use sqlbuild_core::scenario_command::main::snapshot_capture_warning::snapshot_capture_warning;
use sqlbuild_core::scenario_command::main::sql_analysis_required_error::sql_analysis_required_error;
use sqlbuild_core::scenario_command::models::SqlAnalysisCommand;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::scenario_command::conversions::succeeded;
use crate::bindings::_helpers::scenario_command::presentation::{
    CompletionKind, RelayContext, SectionStatus, activity, begin_section, connection_hooks,
    progress_relay, write_completion,
};
use crate::bindings::_helpers::scenario_command::setup::{
    CompileInputs, ScenarioProject, capture_limits, capture_settings, cli_user_error, compile,
    discover_project, flush, project_adapter_name, project_connection_config,
    require_capture_dialect, require_sql_analysis, resolve_adapter, selected_scenarios, style,
    write, write_headers,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const EXECUTION_COMMAND: &str = "sqb scenario capture";

/// Run one `sqb scenario capture` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_scenario_capture_command(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
) -> PyResult<i32> {
    let no_sql_validation: Bound<'_, PyAny> = native_field(request, "no_sql_validation")?;
    if let Some(error) = sql_analysis_required_error(
        SqlAnalysisCommand::Capture,
        no_sql_validation.is_truthy()?,
        None,
    ) {
        return Err(cli_user_error(py, error));
    }
    let project: ScenarioProject<'_> = discover_project(py, request)?;
    require_sql_analysis(&project, (SqlAnalysisCommand::Capture, false))?;
    let adapter_name: String = project_adapter_name(&project)?;
    let adapter: Bound<'_, PyAny> = resolve_adapter(&project, &adapter_name)?;
    let capture_dialect: String = require_capture_dialect(&adapter, &adapter_name)?;
    let connection_config: Bound<'_, PyAny> = project_connection_config(&project)?;
    let style: CliStyle = style(request)?;
    let stream: Bound<'_, PyAny> = py.import("sys")?.getattr("stdout")?;
    let planning: Bound<'_, PyAny> =
        write_headers(&stream, &project, (EXECUTION_COMMAND, 1, style))?;
    let force: bool =
        native_field(&native_field(request, "limit_inputs")?, "force")?.is_truthy()?;
    write(&stream, &format!("{}\n\n", snapshot_capture_warning(force)))?;
    flush(&stream)?;
    let pipeline: Bound<'_, PyAny> = compile(
        &project,
        CompileInputs {
            adapter: &adapter,
            options: &[
                ("no_sql_validation", no_sql_validation),
                ("connection_config", connection_config.clone()),
            ],
            planning: &planning,
        },
    )?;
    let selectors: Vec<String> = project.selectors.extract()?;
    let exclude: Vec<String> = project.exclude.extract()?;
    let scenarios: Bound<'_, PyTuple> =
        selected_scenarios(&pipeline, (&selectors, &exclude, &project.project_dir))?;
    let limits: Bound<'_, PyAny> = capture_limits(request, &project)?;
    let settings: Bound<'_, PyAny> = capture_settings(
        py,
        (
            &adapter_name,
            &capture_dialect,
            native_field(request, "retain")?.is_truthy()?,
        ),
        limits,
    )?;
    let section: SectionStatus<'_> = begin_section(
        &stream,
        (
            &format!("Scenario Capture ({} selected)", scenarios.len()),
            "",
            activity("Capturing scenarios...", &project.namespace),
        ),
        style,
    )?;
    let hooks: Bound<'_, PyAny> = connection_hooks(&stream, (&adapter_name, style))?;
    let relay = progress_relay(
        &stream,
        &section,
        RelayContext {
            project_dir: &project.project_dir,
            adapter: &adapter,
            kind: CompletionKind::Capture,
            style,
            rows: None,
        },
    )?;
    let results: Vec<Bound<'_, PyAny>> = call_with(
        &host_entry(py, ExecutorHost::RunScenarioCapturePipeline)?,
        &[
            ("project_dir", project.project_dir.clone()),
            ("pipeline_result", pipeline),
            ("scenarios", scenarios.into_any()),
            ("connection_config", connection_config),
            ("adapter", adapter.clone()),
            (
                "project_name",
                native_field(
                    &native_field(&project.discovered, "project_config")?,
                    "name",
                )?,
            ),
            ("settings", settings),
            ("connection_hooks", hooks),
            (
                "on_scenario_start",
                native_field(&relay, "on_scenario_start")?,
            ),
            (
                "on_scenario_complete",
                native_field(&relay, "on_scenario_complete")?,
            ),
        ],
    )?
    .try_iter()?
    .collect::<PyResult<_>>()?;
    section.status.call_method0("close")?;
    let mut passed: usize = 0;
    for result in &results {
        if succeeded(result)? {
            passed += 1;
        }
    }
    let failed: usize = results.len() - passed;
    write(
        &stream,
        &format!(
            "\n{}\n",
            format_summary_footer(
                style,
                &[
                    SummaryCount::of("PASS", passed),
                    SummaryCount::of("FAIL", failed),
                    SummaryCount::of("TOTAL", results.len())
                ],
                None
            )
        ),
    )?;
    flush(&stream)?;
    write_completion(&stream, (&project.namespace, failed == 0))?;
    Ok(i32::from(failed != 0))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_scenario_capture_command, module)?)?;
    Ok(())
}
