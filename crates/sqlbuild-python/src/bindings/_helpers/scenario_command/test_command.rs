//! `sqb scenario test`: run the selected scenarios warehouse-direct or replay them locally.

use std::path::PathBuf;
use std::time::Instant;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple, PyTupleMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::definition_changes::models::{ExecutedItem, SelectedItem};
use sqlbuild_core::cli_text::main::format_summary_footer::format_summary_footer;
use sqlbuild_core::cli_text::models::{CliStyle, SummaryCount};
use sqlbuild_core::progress::models::StatementSummary;
use sqlbuild_core::run_on::main::read_run_on_decisions::read_run_on_decisions;
use sqlbuild_core::run_on::main::render_local_refusals::render_local_refusals;
use sqlbuild_core::run_on::main::render_warehouse_census::render_warehouse_census;
use sqlbuild_core::run_on::main::run_on_split::run_on_split;
use sqlbuild_core::run_on::main::with_run_on_json::with_run_on_json;
use sqlbuild_core::run_on::models::{RunOn, RunOnDecision};
use sqlbuild_core::scenario_command::main::render_scenario_failures::render_scenario_failures;
use sqlbuild_core::scenario_command::main::validate_scenario_test_flags::validate_scenario_test_flags;
use sqlbuild_core::scenario_command::models::{
    ScenarioNamespace, ScenarioResultRow, ScenarioTestFlags, SqlAnalysisCommand,
};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::{ExecutionStatus, ScenarioRunResult};
use sqlbuild_host::types::ResultEnum;

use crate::bindings::_helpers::change_selection::base_snapshot::{
    BaseComparison, base_comparison, base_variant,
};
use crate::bindings::_helpers::change_selection::selection::{
    ChangedProject, changed_project, tests_and_models, with_since_section,
};
use crate::bindings::_helpers::native_host::statement_report::{StatementReport, StatementSource};
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
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::build_progress::main::progress::current_projector::current_projector;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::types::ReadResult;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::command_runtime::main::resolve_concurrency::resolve_concurrency;
use sqlbuild_python_front::command_runtime::main::resolve_run_on::resolve_run_on;
use sqlbuild_python_output::result_output::main::results::format_scenario_execution_json_with::format_scenario_execution_json_with;
use sqlbuild_python_output::result_output::main::results::write_execution_json_output::write_execution_json_output;

const LOCAL_ADAPTER: &str = "duckdb";
const STATUSES: [&str; 3] = ["PASS", "FAIL", "ERROR"];
const TARGET_DIR: &str = "target";
const RUN_ON_SETTING: &str = "scenario_run_on";
const SCENARIO_NOUNS: (&str, &str) = ("scenario", "scenarios");
const JSON_SCENARIO_NAME_KEYS: [&str; 2] = ["scenario_name", "name"];

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

fn prepare<'py>(
    request: &Bound<'py, PyAny>,
    project: ScenarioProject<'py>,
    local: bool,
) -> PyResult<TestRun<'py>> {
    let py: Python<'py> = request.py();
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

/// Where a run sent its scenarios, for the summary, the refusal report and `--json`.
struct RunOnReport {
    mode: RunOn,
    decisions: Vec<RunOnDecision>,
    local_statements: StatementSummary,
    verbose: bool,
}

/// The run's elapsed summary, refusals, completion line and JSON document.
fn finish(
    request: &Bound<'_, PyAny>,
    run: &TestRun<'_>,
    (section, started, results): (
        &SectionStatus<'_>,
        Instant,
        &[ScenarioRunResult<Bound<'_, PyAny>>],
    ),
    report: &RunOnReport,
) -> PyResult<bool> {
    let py: Python<'_> = request.py();
    section.status.call_method0("close")?;
    let elapsed_seconds: f64 = started.elapsed().as_secs_f64();
    let mut tallies: [usize; 3] = [0; 3];
    for result in results {
        let status: String = match result.local_status {
            Some(status) => status.value().to_owned(),
            None if result.status == ExecutionStatus::Success => STATUSES[0].to_owned(),
            None => STATUSES[1].to_owned(),
        };
        if let Some(index) = STATUSES.iter().position(|label| *label == status) {
            tallies[index] += 1;
        }
    }
    let mut counts: Vec<SummaryCount> = STATUSES
        .iter()
        .zip(tallies)
        .filter(|(label, value)| **label != STATUSES[2] || *value > 0)
        .map(|(label, value)| count(label, value))
        .collect();
    counts.push(count("TOTAL", results.len()));
    let succeeded: bool = tallies[1] == 0 && tallies[2] == 0;
    let local: usize = results
        .iter()
        .filter(|result| result.local_status.is_some())
        .count();
    let split: (usize, usize) = (local, results.len() - local);
    write(
        &run.stream,
        &format!(
            "\n{}\n",
            format_summary_footer(
                run.style,
                &counts,
                Some(&format!(
                    "{}, {elapsed_seconds:.2}s",
                    run_on_split(split.0, split.1)
                ))
            )
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
    let statements: Option<StatementReport> = StatementReport::read_excluding(
        py,
        &StatementSource {
            adapter_name: &run.adapter_name,
            adapter: &run.adapter,
            connection_config: &run.connection_config,
            started,
        },
        &report.local_statements,
    )?;
    if let Some(line) = statements.as_ref().and_then(StatementReport::line) {
        write(&run.stream, &format!("{line}\n"))?;
    }
    let refusals: String = match report.mode {
        RunOn::Warehouse => String::new(),
        RunOn::Local => render_local_refusals(&report.decisions, true, SCENARIO_NOUNS, run.style),
        RunOn::Auto if report.verbose => {
            render_local_refusals(&report.decisions, false, SCENARIO_NOUNS, run.style)
        }
        RunOn::Auto => render_warehouse_census(&report.decisions, SCENARIO_NOUNS, run.style),
    };
    if !refusals.is_empty() {
        write(&run.stream, &refusals)?;
    }
    flush(&run.stream)?;
    write_completion(&run.stream, (&run.project.namespace, succeeded))?;
    let namespace: &ScenarioNamespace = &run.project.namespace;
    let payload: String = with_since_section(
        format_scenario_execution_json_with(
            py,
            results,
            Some(keywords(
                py,
                &[
                    ("local", boolean(py, report.mode == RunOn::Local)),
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
    )?;
    let payload: String = with_run_on_json(
        &payload,
        ("scenarios", &JSON_SCENARIO_NAME_KEYS),
        &report.decisions,
        split,
    )
    .map_err(value_error)?;
    let payload: String = match &statements {
        Some(statements) => statements.with_section(&payload)?,
        None => payload,
    };
    write_execution_json_output(
        py,
        PyString::new(py, &payload).into_any(),
        request.getattr("json_output")?.extract()?,
        request.getattr("json_output_path")?,
    )?;
    Ok(succeeded)
}

fn count(label: &str, value: usize) -> SummaryCount {
    SummaryCount {
        label: label.to_owned(),
        value: i64::try_from(value).unwrap_or(i64::MAX),
    }
}

/// Run `scenarios` locally on run-scoped DuckDB or on the warehouse, as `run` is set up.
fn execute_batch<'py>(
    request: &Bound<'py, PyAny>,
    run: &TestRun<'py>,
    (pipeline, scenarios): (&Bound<'py, PyAny>, &Bound<'py, PyTuple>),
    section: &SectionStatus<'py>,
) -> PyResult<Vec<ScenarioRunResult<Bound<'py, PyAny>>>> {
    let py: Python<'py> = request.py();
    if scenarios.is_empty() {
        return Ok(Vec::new());
    }
    let target_dir: PathBuf = run
        .project
        .project_dir
        .extract::<PathBuf>()?
        .join(TARGET_DIR);
    let relay = progress_relay(
        &run.stream,
        section,
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
        arguments.extend([
            ("project_dir", run.project.project_dir.clone()),
            ("replay_source", replay_source(run)?),
        ]);
        ExecutorHost::RunScenarioLocalTestPipeline
    } else {
        arguments.extend([
            ("connection_config", run.connection_config.clone()),
            (
                "settings",
                call_with(
                    &host_entry(py, ExecutorHost::ScenarioTestSettings)?,
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
    Vec::<ScenarioRunResult<_>>::read(&call_with(&host_entry(py, runner)?, &arguments)?)
}

/// The project adapter whose dialect locally replayed SQL is authored in.
fn replay_source<'py>(run: &TestRun<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = run.stream.py();
    call_with(
        &host_entry(py, ExecutorHost::ScenarioLocalReplaySource)?,
        &[
            (
                "lexical_syntax",
                run.project_adapter.getattr("sql_lexical_syntax")?,
            ),
            ("capture_adapter", text(py, &run.project_adapter_name)),
            ("capture_dialect", text(py, &run.capture_dialect)),
        ],
    )
}

/// Each selected scenario's run-on decision, from the local runner.
fn decide<'py>(
    run: &TestRun<'py>,
    (pipeline, scenarios): (&Bound<'py, PyAny>, &Bound<'py, PyTuple>),
    mode: RunOn,
) -> PyResult<Vec<RunOnDecision>> {
    let py: Python<'py> = run.stream.py();
    let project_name: Bound<'_, PyAny> = native_field(
        &native_field(&run.project.discovered, "project_config")?,
        "name",
    )?;
    let json: String = call_with(
        &host_entry(py, ExecutorHost::DecideScenarioRunOn)?,
        &[
            ("project_dir", run.project.project_dir.clone()),
            ("pipeline_result", pipeline.clone()),
            ("scenarios", scenarios.clone().into_any()),
            ("adapter", run.adapter.clone()),
            ("project_name", project_name),
            ("replay_source", replay_source(run)?),
            ("run_on", text(py, mode.as_str())),
        ],
    )?
    .extract()?;
    read_run_on_decisions(&json).map_err(value_error)
}

/// The statements the installed projector has counted so far.
fn statement_summary(py: Python<'_>) -> PyResult<StatementSummary> {
    match current_projector(py)? {
        Some(projector) => projector.get().statement_summary(),
        None => Ok(StatementSummary::default()),
    }
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

/// A compiled, selected and `--since`-narrowed set of scenarios, and the `--since` JSON.
fn compiled<'py>(
    request: &Bound<'py, PyAny>,
    run: &TestRun<'py>,
    planning: &Bound<'py, PyAny>,
    base: Option<BaseComparison>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyTuple>, Option<String>)> {
    let py: Python<'py> = request.py();
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
            planning,
        },
    );
    let finished: PyResult<Bound<'_, PyAny>> = planning.call_method0("finish");
    let pipeline: Bound<'_, PyAny> = compiled?;
    finished?;
    let selectors: Vec<String> = run.project.selectors.extract()?;
    let exclude: Vec<String> = run.project.exclude.extract()?;
    let mut scenarios: Bound<'_, PyTuple> =
        selected_scenarios(&pipeline, (&selectors, &exclude, &run.project.project_dir))?;
    let mut since_json: Option<String> = None;
    if let Some(base) = base {
        let changed_json: String;
        (scenarios, changed_json) =
            select_changed(run, (&pipeline, scenarios), base, flag(request, "explain")?)?;
        since_json = Some(changed_json);
    }
    Ok((pipeline, scenarios, since_json))
}

/// The `--since` base of a run, when the request names one.
fn since_base(request: &Bound<'_, PyAny>, run: &TestRun<'_>) -> PyResult<Option<BaseComparison>> {
    let py: Python<'_> = request.py();
    let since: Option<String> = native_field(request, "since")?.extract()?;
    since
        .as_deref()
        .map(|since| {
            base_comparison(
                (&run.stream, &run.project.project_dir),
                since,
                &base_variant(None, &py.None().into_bound(py))?,
            )
        })
        .transpose()
}

/// The scenarios of `scenarios` whose name is in `names`, in order.
fn named<'py>(scenarios: &Bound<'py, PyTuple>, names: &[&str]) -> PyResult<Bound<'py, PyTuple>> {
    let mut kept: Vec<Bound<'py, PyAny>> = Vec::new();
    for scenario in scenarios.iter() {
        let name: String = native_field(&scenario, "name")?.extract()?;
        if names.contains(&name.as_str()) {
            kept.push(scenario);
        }
    }
    PyTuple::new(scenarios.py(), kept)
}

/// Run one `sqb scenario test` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_scenario_test_command(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
) -> PyResult<i32> {
    let project: ScenarioProject<'_> = discover_project(py, request)?;
    let mode: RunOn = resolve_run_on(
        &request.getattr("run_on")?,
        &project.discovered,
        &py.None().into_bound(py),
        RUN_ON_SETTING,
    )?;
    validate_scenario_test_flags(ScenarioTestFlags {
        local: mode == RunOn::Local,
        warehouse: mode == RunOn::Warehouse,
        retain: flag(request, "retain")?,
        sync_snapshots: flag(request, "sync_snapshots")?,
        refresh: flag(request, "refresh")?,
    })
    .map_err(|error| cli_user_error(py, error))?;
    let mut run: TestRun<'_> = prepare(request, project, mode != RunOn::Warehouse)?;
    let planning: Bound<'_, PyAny> = write_headers(
        &run.stream,
        &run.project,
        (
            "sqb scenario test",
            if mode == RunOn::Local {
                1
            } else {
                run.concurrency
            },
            run.style,
        ),
    )?;
    let base: Option<BaseComparison> = since_base(request, &run)?;
    let (pipeline, scenarios, since_json) = compiled(request, &run, &planning, base)?;
    run.since_json = since_json;
    if run.local && (flag(request, "sync_snapshots")? || flag(request, "refresh")?) {
        let synced: LocalSync<'_> = sync_local_snapshots(request, &run, (&pipeline, &scenarios))?;
        if synced.exit_code != 0 {
            return synced.write_failure_json(request, &run.project.namespace);
        }
    }
    let decisions: Vec<RunOnDecision> = if run.local {
        decide(&run, (&pipeline, &scenarios), mode)?
    } else {
        scenarios
            .iter()
            .map(|scenario| {
                Ok(RunOnDecision {
                    name: native_field(&scenario, "name")?.extract()?,
                    run_on: "warehouse".to_owned(),
                    refusals: Vec::new(),
                })
            })
            .collect::<PyResult<_>>()?
    };
    let local_names: Vec<&str> = decisions
        .iter()
        .filter(|decision| mode == RunOn::Local || decision.local())
        .map(|decision| decision.name.as_str())
        .collect();
    let warehouse_names: Vec<&str> = decisions
        .iter()
        .filter(|decision| mode != RunOn::Local && !decision.local())
        .map(|decision| decision.name.as_str())
        .collect();
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
    let mut results: Vec<ScenarioRunResult<Bound<'_, PyAny>>> = Vec::new();
    let mut local_statements: StatementSummary = StatementSummary::default();
    if run.local {
        let before: StatementSummary = statement_summary(py)?;
        results.extend(execute_batch(
            request,
            &run,
            (&pipeline, &named(&scenarios, &local_names)?),
            &section,
        )?);
        local_statements = statement_delta(&before, &statement_summary(py)?);
    }
    let reporting: TestRun<'_> = if warehouse_names.is_empty() || !run.local {
        if !run.local {
            results.extend(execute_batch(
                request,
                &run,
                (&pipeline, &named(&scenarios, &warehouse_names)?),
                &section,
            )?);
        }
        run
    } else {
        let mut remote: TestRun<'_> = prepare(request, discover_project(py, request)?, false)?;
        let quiet: Bound<'_, PyAny> = Bound::new(
            py,
            sqlbuild_python_output::build_progress::models::PlanningProgressReporter::new(
                py,
                &remote.stream,
                remote.style.use_color,
            )?,
        )?
        .into_any();
        let remote_base: Option<BaseComparison> = since_base(request, &remote)?;
        let (remote_pipeline, remote_scenarios, _remote_since_json) =
            compiled(request, &remote, &quiet, remote_base)?;
        remote.since_json = run.since_json.clone();
        results.extend(execute_batch(
            request,
            &remote,
            (
                &remote_pipeline,
                &named(&remote_scenarios, &warehouse_names)?,
            ),
            &section,
        )?);
        remote
    };
    let succeeded: bool = finish(
        request,
        &reporting,
        (&section, started, &results),
        &RunOnReport {
            mode,
            decisions,
            local_statements,
            verbose: flag(request, "verbose")?,
        },
    )?;
    Ok(if succeeded { 0 } else { 1 })
}

/// The statements counted between `before` and `after`.
fn statement_delta(before: &StatementSummary, after: &StatementSummary) -> StatementSummary {
    let mut delta: StatementSummary = StatementSummary::default();
    for (kind, count) in &after.by_kind {
        let earlier: u64 = before.by_kind.get(kind).copied().unwrap_or(0);
        if *count > earlier {
            delta.by_kind.insert(kind.clone(), count - earlier);
        }
    }
    delta.query_ids = after
        .query_ids
        .iter()
        .filter(|query_id| !before.query_ids.contains(query_id))
        .cloned()
        .collect();
    delta
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_scenario_test_command, module)?)?;
    Ok(())
}
