//! `sqb scenario test`: run the selected scenarios warehouse-direct or replay them locally.

use std::path::PathBuf;
use std::time::Instant;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple, PyTupleMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::definition_changes::models::{ExecutedItem, SelectedItem};
use sqlbuild_core::cli_text::main::format_summary_footer::format_summary_footer;
use sqlbuild_core::cli_text::models::{CliStyle, SummaryCount};
use sqlbuild_core::scenario_command::main::render_scenario_failures::render_scenario_failures;
use sqlbuild_core::scenario_command::main::validate_scenario_test_flags::validate_scenario_test_flags;
use sqlbuild_core::scenario_command::models::{
    ScenarioNamespace, ScenarioResultRow, ScenarioTestFlags, SqlAnalysisCommand,
};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::{ExecutionStatus, ScenarioRunResult};
use sqlbuild_host::types::ResultEnum;

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::change_selection::base_snapshot::{
    BaseComparison, base_comparison, base_variant,
};
use crate::bindings::_helpers::change_selection::selection::{
    ChangedProject, changed_project, tests_and_models, with_since_section,
};
use crate::bindings::_helpers::command_runtime::concurrency::resolve_concurrency;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::results::ReadResult;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::result_output::documents::format_scenario_execution_json_with;
use crate::bindings::_helpers::result_output::documents::write_execution_json_output;
use crate::bindings::_helpers::scenario_command::conversions::scenario_result_row;
use crate::bindings::_helpers::scenario_command::presentation::{
    CompletionKind, RelayContext, SectionStatus, activity, begin_section, connection_hooks,
    progress_relay, write_completion,
};
use crate::bindings::_helpers::scenario_command::setup::{
    CompileInputs, ScenarioProject, boolean, cli_user_error, compile, discover_project, flush, int,
    optional_text, project_adapter_name, project_connection_config, require_capture_dialect,
    require_sql_analysis, resolve_adapter, selected_scenarios, style, text, write, write_headers,
};
use crate::bindings::_helpers::scenario_command::snapshot_sync::{LocalSync, sync_local_snapshots};
use crate::bindings::_helpers::sql_test_command::test_command::timeout_record;
use crate::bindings::_helpers::sql_test_command::timeouts::{TimeoutKind, resolve_run_timeout};

const LOCAL_ADAPTER: &str = "duckdb";
const LOCAL_STATUSES: [&str; 4] = ["PASS", "FAIL", "ERROR", "SKIP"];
const TARGET_DIR: &str = "target";

/// The resolved adapters, connection and output settings of one scenario test run.
pub(crate) struct TestRun<'py> {
    pub(crate) project: ScenarioProject<'py>,
    pub(crate) local: bool,
    pub(crate) adapter_name: String,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) project_adapter_name: String,
    pub(crate) project_adapter: Bound<'py, PyAny>,
    pub(crate) capture_dialect: String,
    pub(crate) connection_config: Bound<'py, PyAny>,
    pub(crate) stream: Bound<'py, PyAny>,
    pub(crate) style: CliStyle,
    pub(crate) concurrency: i64,
    /// The `since` JSON section of a `--since` run.
    pub(crate) since_json: Option<String>,
}

fn flag(request: &Bound<'_, PyAny>, name: &str) -> PyResult<bool> {
    native_field(request, name)?.is_truthy()
}

fn prepare<'py>(py: Python<'py>, request: &Bound<'py, PyAny>) -> PyResult<TestRun<'py>> {
    let local: bool = flag(request, "local")?;
    validate_scenario_test_flags(ScenarioTestFlags {
        local,
        retain: flag(request, "retain")?,
        sync_snapshots: flag(request, "sync_snapshots")?,
        refresh: flag(request, "refresh")?,
    })
    .map_err(|error| cli_user_error(py, error))?;
    let project: ScenarioProject<'py> = discover_project(py, request)?;
    if local {
        require_sql_analysis(
            &project,
            (
                SqlAnalysisCommand::LocalTest,
                flag(request, "no_sql_validation")?,
            ),
        )?;
    }
    let project_adapter_name: String = project_adapter_name(&project)?;
    let adapter_name: String = if local {
        LOCAL_ADAPTER.to_owned()
    } else {
        project_adapter_name.clone()
    };
    let adapter: Bound<'py, PyAny> = resolve_adapter(&project, &adapter_name)?;
    let project_adapter: Bound<'py, PyAny> = resolve_adapter(&project, &project_adapter_name)?;
    let capture_dialect: String = if local {
        require_capture_dialect(&project_adapter, &project_adapter_name)?
    } else {
        String::new()
    };
    let connection_config: Bound<'py, PyAny> = if local {
        let memory: Bound<'py, PyDict> = PyDict::new(py);
        memory.set_item("database", ":memory:")?;
        memory.into_any()
    } else {
        project_connection_config(&project)?
    };
    let style: CliStyle = style(request)?;
    let concurrency: i64 = resolve_concurrency(
        &request.getattr("concurrency")?,
        &project.discovered,
        &py.None().into_bound(py),
    )?
    .limit;
    let sys: Bound<'py, PyModule> = py.import("sys")?;
    let stream: Bound<'py, PyAny> = sys.getattr(if flag(request, "json_output")? {
        "stderr"
    } else {
        "stdout"
    })?;
    Ok(TestRun {
        project,
        local,
        adapter_name,
        adapter,
        project_adapter_name,
        project_adapter,
        capture_dialect,
        connection_config,
        stream,
        style,
        concurrency: concurrency.max(1),
        since_json: None,
    })
}

/// The run's elapsed summary, completion line and JSON document.
fn finish(
    request: &Bound<'_, PyAny>,
    run: &TestRun<'_>,
    (section, started, results): (
        &SectionStatus<'_>,
        Instant,
        &[ScenarioRunResult<Bound<'_, PyAny>>],
    ),
    (counts, succeeded): (Vec<SummaryCount>, bool),
) -> PyResult<()> {
    let py: Python<'_> = request.py();
    section.status.call_method0("close")?;
    let elapsed_seconds: f64 = started.elapsed().as_secs_f64();
    write(
        &run.stream,
        &format!(
            "\n{}\n",
            format_summary_footer(run.style, &counts, Some(&format!("{elapsed_seconds:.2}s")))
        ),
    )?;
    let rows: Vec<ScenarioResultRow> = results
        .iter()
        .map(|result| scenario_result_row(py, result))
        .collect::<PyResult<_>>()?;
    let failures: String = render_scenario_failures(&rows, run.style);
    if !failures.is_empty() {
        write(&run.stream, &format!("{failures}\n"))?;
    }
    flush(&run.stream)?;
    write_completion(&run.stream, (&run.project.namespace, succeeded))?;
    let namespace: &ScenarioNamespace = &run.project.namespace;
    let payload: Bound<'_, PyAny> = PyString::new(
        py,
        &with_since_section(
            format_scenario_execution_json_with(
                py,
                results,
                Some(keywords(
                    py,
                    &[
                        ("local", boolean(py, run.local)),
                        (
                            "run_namespace",
                            optional_text(py, namespace.value.as_deref()),
                        ),
                        ("namespace_source", text(py, &namespace.source)),
                        (
                            "duration_ms",
                            pyo3::types::PyFloat::new(py, elapsed_seconds * 1000.0).into_any(),
                        ),
                    ],
                )?),
            )?,
            run.since_json.as_deref(),
        )?,
    )
    .into_any();
    write_execution_json_output(
        py,
        payload,
        request.getattr("json_output")?.extract()?,
        request.getattr("json_output_path")?,
    )?;
    Ok(())
}

fn count(label: &str, value: usize) -> SummaryCount {
    SummaryCount {
        label: label.to_owned(),
        value: i64::try_from(value).unwrap_or(i64::MAX),
    }
}

/// Run the selected scenarios warehouse-direct, or replay them on run-scoped DuckDB.
fn run_scenarios(
    request: &Bound<'_, PyAny>,
    run: &TestRun<'_>,
    (pipeline, scenarios): (&Bound<'_, PyAny>, &Bound<'_, PyTuple>),
) -> PyResult<i32> {
    let py: Python<'_> = request.py();
    let section: SectionStatus<'_> = begin_section(
        &run.stream,
        (
            &format!("Scenario ({} selected)", scenarios.len()),
            "",
            activity("Running scenarios...", &run.project.namespace),
        ),
        run.style,
    )?;
    let started: Instant = Instant::now();
    let target_dir: PathBuf = run
        .project
        .project_dir
        .extract::<PathBuf>()?
        .join(TARGET_DIR);
    let relay = progress_relay(
        &run.stream,
        &section,
        RelayContext {
            project_dir: &run.project.project_dir,
            adapter: &run.adapter,
            kind: CompletionKind::Run { target_dir },
            style: run.style,
        },
    )?;
    let project_name: Bound<'_, PyAny> = native_field(
        &native_field(&run.project.discovered, "project_config")?,
        "name",
    )?;
    let mut arguments: Vec<(&str, Bound<'_, PyAny>)> = vec![
        ("pipeline_result", pipeline.clone()),
        ("scenarios", scenarios.clone().into_any()),
        ("adapter", run.adapter.clone()),
        ("project_name", project_name),
        ("on_scenario_start", relay.getattr("on_scenario_start")?),
        (
            "on_scenario_complete",
            relay.getattr("on_scenario_complete")?,
        ),
    ];
    let runner: ExecutorHost = if run.local {
        let replay_source: Bound<'_, PyAny> = call_with(
            &host::entry(py, ExecutorHost::ScenarioLocalReplaySource)?,
            &[
                (
                    "lexical_syntax",
                    run.project_adapter.getattr("sql_lexical_syntax")?,
                ),
                ("capture_adapter", text(py, &run.project_adapter_name)),
                ("capture_dialect", text(py, &run.capture_dialect)),
            ],
        )?;
        arguments.extend([
            ("project_dir", run.project.project_dir.clone()),
            ("strict", request.getattr("strict")?),
            ("replay_source", replay_source),
        ]);
        ExecutorHost::RunScenarioLocalTestPipeline
    } else {
        arguments.extend([
            ("connection_config", run.connection_config.clone()),
            (
                "settings",
                call_with(
                    &host::entry(py, ExecutorHost::ScenarioTestSettings)?,
                    &[
                        ("retain", request.getattr("retain")?),
                        ("concurrency", int(py, run.concurrency)),
                        (
                            "timeouts",
                            timeout_record(
                                py,
                                &resolve_run_timeout(
                                    (&native_field(pipeline, "project")?, &run.project.discovered),
                                    request.getattr("scenario_timeout")?.extract()?,
                                    TimeoutKind::Scenario,
                                )?,
                            )?,
                        ),
                    ],
                )?,
            ),
            (
                "connection_hooks",
                connection_hooks(&run.stream, (&run.adapter_name, run.style))?,
            ),
        ]);
        ExecutorHost::RunScenarioTestPipeline
    };
    let results =
        Vec::<ScenarioRunResult<_>>::read(&call_with(&host::entry(py, runner)?, &arguments)?)?;
    let total: usize = results.len();
    let (counts, ok): (Vec<SummaryCount>, bool) = if run.local {
        let mut tallies: [usize; 4] = [0; 4];
        for result in results.iter() {
            let Some(status) = result.local_status else {
                continue;
            };
            let value = status.value();
            if let Some(index) = LOCAL_STATUSES.iter().position(|label| *label == value) {
                tallies[index] += 1;
            }
        }
        let mut counts: Vec<SummaryCount> = LOCAL_STATUSES
            .iter()
            .zip(tallies)
            .map(|(label, value)| count(label, value))
            .collect();
        counts.push(count("TOTAL", total));
        (counts, tallies[1] == 0 && tallies[2] == 0)
    } else {
        let mut passed: usize = 0;
        for result in results.iter() {
            if result.status == ExecutionStatus::Success {
                passed += 1;
            }
        }
        (
            vec![
                count("PASS", passed),
                count("FAIL", total - passed),
                count("TOTAL", total),
            ],
            passed == total,
        )
    };
    finish(request, run, (&section, started, &results), (counts, ok))?;
    Ok(if ok { 0 } else { 1 })
}

/// Keep only the scenarios that execute something changed since the base revision.
fn select_changed<'py>(
    run: &TestRun<'py>,
    (pipeline, scenarios): (&Bound<'py, PyAny>, Bound<'py, PyTuple>),
    base: BaseComparison,
    explain: bool,
) -> PyResult<(Bound<'py, PyTuple>, String)> {
    let py: Python<'py> = pipeline.py();
    let project: Bound<'py, PyAny> = native_field(pipeline, "project")?;
    let changed: ChangedProject = changed_project(
        (&project, &run.project.discovered),
        &run.project.project_dir.extract::<PathBuf>()?,
        base,
    )?;
    let (_, models) = tests_and_models(&project)?;
    let mut kept: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut selected: Vec<SelectedItem> = Vec::new();
    for scenario in scenarios.iter() {
        let item: ExecutedItem = changed.scenario_item(&scenario, (&project, &models))?;
        let reasons: Vec<String> = changed.reasons(&item);
        if reasons.is_empty() {
            continue;
        }
        selected.push(SelectedItem {
            label: native_field(&scenario, "name")?.extract()?,
            reasons,
        });
        kept.push(scenario);
    }
    changed.report(
        &run.stream,
        ("scenario", "scenarios"),
        (scenarios.len(), &selected, explain),
    )?;
    Ok((
        PyTuple::new(py, kept)?,
        changed.json((scenarios.len(), &selected)),
    ))
}

/// Run one `sqb scenario test` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_scenario_test_command(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
) -> PyResult<i32> {
    let mut run: TestRun<'_> = prepare(py, request)?;
    let planning: Bound<'_, PyAny> = write_headers(
        &run.stream,
        &run.project,
        (
            if run.local {
                "sqb scenario test --local"
            } else {
                "sqb scenario test"
            },
            if run.local { 1 } else { run.concurrency },
            run.style,
        ),
    )?;
    let since: Option<String> = native_field(request, "since")?.extract()?;
    let base: Option<BaseComparison> = match since.as_deref() {
        Some(since) => Some(base_comparison(
            (&run.stream, &run.project.project_dir),
            since,
            &base_variant(None, &py.None().into_bound(py))?,
        )?),
        None => None,
    };
    let no_sql_validation: bool = run.local || flag(request, "no_sql_validation")?;
    let compiled: PyResult<Bound<'_, PyAny>> = compile(
        &run.project,
        CompileInputs {
            adapter: &run.adapter,
            options: &[
                ("no_sql_validation", boolean(py, no_sql_validation)),
                ("source_deferral_enabled", boolean(py, false)),
                ("connection_config", run.connection_config.clone()),
                (
                    "source_lexical_syntax",
                    native_field(&run.project_adapter, "sql_lexical_syntax")?,
                ),
            ],
            planning: &planning,
        },
    );
    let finished: PyResult<Bound<'_, PyAny>> = planning.call_method0("finish");
    let pipeline: Bound<'_, PyAny> = compiled?;
    finished?;
    let selectors: Vec<String> = run.project.selectors.extract()?;
    let exclude: Vec<String> = run.project.exclude.extract()?;
    let mut scenarios: Bound<'_, PyTuple> =
        selected_scenarios(&pipeline, (&selectors, &exclude, &run.project.project_dir))?;
    if let Some(base) = base {
        let since_json: String;
        (scenarios, since_json) = select_changed(
            &run,
            (&pipeline, scenarios),
            base,
            flag(request, "explain")?,
        )?;
        run.since_json = Some(since_json);
    }
    if run.local && (flag(request, "sync_snapshots")? || flag(request, "refresh")?) {
        let synced: LocalSync<'_> = sync_local_snapshots(request, &run, (&pipeline, &scenarios))?;
        if synced.exit_code != 0 {
            return synced.write_failure_json(request, &run.project.namespace);
        }
    }
    run_scenarios(request, &run, (&pipeline, &scenarios))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_scenario_test_command, module)?)?;
    Ok(())
}
