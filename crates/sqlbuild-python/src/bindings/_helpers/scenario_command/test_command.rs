//! `sqb scenario test`: run the selected scenarios warehouse-direct or replay them locally.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple, PyTupleMethods};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_analysis::definition_changes::models::{ExecutedItem, SelectedItem};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::progress::main::statement_summary_delta::statement_summary_delta;
use sqlbuild_core::progress::models::ExpectationStep;
use sqlbuild_core::progress::models::StatementSummary;
use sqlbuild_core::run_on::main::read_run_on_decisions::read_run_on_decisions;
use sqlbuild_core::run_on::main::refusal_causes::refusal_causes;
use sqlbuild_core::run_on::main::refusal_explain_rows::refusal_explain_rows;
use sqlbuild_core::run_on::main::render_refusal_report::render_refusal_report;
use sqlbuild_core::run_on::main::with_run_on_json::with_run_on_json;
use sqlbuild_core::run_on::models::{RunOn, RunOnDecision};
use sqlbuild_core::run_output::main::render_failed_list::render_failed_list;
use sqlbuild_core::run_output::main::render_failures::render_failures;
use sqlbuild_core::run_output::main::render_run_completion::render_run_completion;
use sqlbuild_core::run_output::main::render_run_header::render_run_header;
use sqlbuild_core::run_output::models::{
    FailureEntry, FailureField, HeaderCause, RunCounts, RunHeader,
};
use sqlbuild_core::scenario_command::main::validate_scenario_test_flags::validate_scenario_test_flags;
use sqlbuild_core::scenario_command::models::{
    CodedText, ScenarioNamespace, ScenarioResultRow, ScenarioTestFlags, SqlAnalysisCommand,
};
use sqlbuild_core::test_command::main::test_failure_entry::test_failure_entry;
use sqlbuild_core::test_command::models::TestFailureFacts;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::ScenarioRunResult;
use sqlbuild_python_base::build_progress::main::progress::current_native_progress_projector::current_native_progress_projector;
use sqlbuild_python_base::build_progress::main::terminal::terminal_columns::terminal_columns;
use sqlbuild_python_output::build_progress::main::local_attempt::local_attempt;
use sqlbuild_python_output::build_progress::models::LocalAttemptStep;
use sqlbuild_python_output::build_progress::models::NestedCommandProgressCallbacks;
use sqlbuild_python_output::result_output::main::results::split_scenario_fallbacks::split_scenario_fallbacks;

use crate::bindings::_helpers::change_selection::base_snapshot::{
    BaseComparison, base_comparison, base_variant,
};
use crate::bindings::_helpers::change_selection::selection::{
    ChangedProject, changed_project, tests_and_models, with_since_section,
};
use crate::bindings::_helpers::native_host::statement_report::{StatementReport, StatementSource};
use crate::bindings::_helpers::scenario_command::conversions::scenario_result_row;
use crate::bindings::_helpers::scenario_command::presentation::{
    CompletionKind, RelayContext, SectionStatus, connection_hooks, progress_relay, quiet_section,
};
use crate::bindings::_helpers::scenario_command::setup::{
    CompileInputs, ScenarioProject, boolean, cli_user_error, compile, discover_project, flush, int,
    optional_text, planning_reporter, project_adapter_name, project_connection_config,
    require_capture_dialect, require_sql_analysis, resolve_adapter, selected_scenarios, style,
    text, write, write_execution,
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
/// The rollup line after the rows names at most this many causes.
const ROLLUP_CAUSES: usize = 5;

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

/// The reporter that draws a test run's rows and footer, and the scenarios that can't run locally.
struct RowReporter<'py> {
    progress: Bound<'py, PyAny>,
    refused: HashSet<String>,
    local_only: bool,
}

/// Where a run sent its scenarios, for the summary, the refusal report and `--json`.
struct RunOnReport {
    mode: RunOn,
    decisions: Vec<RunOnDecision>,
    local_statements: StatementSummary,
    verbose: bool,
}

/// The header before a run: how many scenarios run where, the causes, and `--explain`'s rows.
fn run_header(decisions: &[RunOnDecision], mode: RunOn, explain: bool) -> RunHeader {
    let refused: usize = decisions
        .iter()
        .filter(|decision| !decision.refusals.is_empty())
        .count();
    let local: usize = decisions
        .iter()
        .filter(|decision| decision.local() && decision.refusals.is_empty())
        .count();
    let local_only: bool = mode == RunOn::Local;
    RunHeader {
        title: "Scenarios".to_owned(),
        nouns: (SCENARIO_NOUNS.0.to_owned(), SCENARIO_NOUNS.1.to_owned()),
        selected: decisions.len(),
        local: if local_only {
            decisions.len() - refused
        } else {
            local
        },
        warehouse: if local_only {
            0
        } else {
            decisions.len() - local
        },
        not_run: if local_only { refused } else { 0 },
        local_only,
        causes: if mode == RunOn::Warehouse {
            Vec::new()
        } else {
            refusal_causes(decisions)
        },
        explain: if explain && mode == RunOn::Auto {
            refusal_explain_rows(decisions)
        } else {
            Vec::new()
        },
    }
}

/// The longest scenario name.
fn name_width(scenarios: &Bound<'_, PyTuple>) -> PyResult<i64> {
    let mut width: usize = 0;
    for scenario in scenarios.iter() {
        width = width.max(
            native_field(&scenario, "name")?
                .extract::<String>()?
                .chars()
                .count(),
        );
    }
    Ok(i64::try_from(width).unwrap_or(i64::MAX))
}

/// `  namespace  <value> (from <source>)` when the run is namespaced; empty when it isn't.
fn namespace_line(namespace: &ScenarioNamespace, style: CliStyle) -> String {
    match &namespace.value {
        Some(value) => format!(
            "{}\n",
            style.muted(&format!("  namespace  {value} (from {})", namespace.source))
        ),
        None => String::new(),
    }
}

/// Each expected and assertion row of a scenario as the failure detail reads it.
fn scenario_steps(row: &ScenarioResultRow) -> Vec<ExpectationStep> {
    let outcome = |failed: bool| if failed { "fail" } else { "pass" }.to_owned();
    let mut steps: Vec<ExpectationStep> = row
        .expected
        .iter()
        .map(|expected| ExpectationStep {
            model_name: expected.model_name.clone(),
            outcome: outcome(expected.failed),
            actual_row_count: expected.actual_row_count.unwrap_or(0),
            expected_row_count: expected.expected_row_count.unwrap_or(0),
            mismatched_row_count: expected.mismatched_row_count.unwrap_or(0),
            missing_row_count: expected.missing_row_count,
            ..ExpectationStep::default()
        })
        .collect();
    steps.extend(row.assertions.iter().map(|assertion| {
        ExpectationStep {
            model_name: format!("assertion {}", assertion.name),
            outcome: outcome(assertion.failed),
            actual_row_count: assertion.failing_row_count.unwrap_or(0),
            unexpected_samples: assertion
                .first_sample
                .iter()
                .map(|sample| vec![("row".to_owned(), sample.clone())])
                .collect(),
            ..ExpectationStep::default()
        }
    }));
    steps
}

/// `Retained (N):` with each retained scenario's local DuckDB and relations; empty with none.
fn retained_section(rows: &[ScenarioResultRow], style: CliStyle) -> String {
    let retained: Vec<&ScenarioResultRow> = rows.iter().filter(|row| row.retained).collect();
    if retained.is_empty() {
        return String::new();
    }
    let mut lines: Vec<String> = vec![
        String::new(),
        style.section(&format!("Retained ({}):", retained.len())),
    ];
    for row in retained {
        lines.push(format!("  {}", row.name));
        if let Some(path) = &row.local_duckdb_path {
            lines.push(format!("    {}{path}", style.muted("duckdb    ")));
        }
        for relation in row.retained_relations.iter().flatten() {
            lines.push(format!(
                "    {}{} -> {}",
                style.muted(&format!("{:<10}", relation.kind)),
                relation.logical_name,
                relation.physical_name
            ));
        }
    }
    format!("{}\n", lines.join("\n"))
}

/// The run's failures, refusals, retained artifacts, failed list and summary, then its JSON document.
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
    let rows: Vec<ScenarioResultRow> = results
        .iter()
        .map(|result| scenario_result_row(py, result))
        .collect::<PyResult<_>>()?;
    let mut counts: RunCounts = RunCounts {
        total: results.len(),
        ..RunCounts::default()
    };
    let mut entries: Vec<FailureEntry> = Vec::new();
    let mut failed: Vec<(String, String)> = Vec::new();
    let mut not_run: usize = 0;
    for (result, row) in results.iter().zip(&rows) {
        let refused: bool = report.mode == RunOn::Local
            && (result.local_refusal.is_some()
                || report
                    .decisions
                    .iter()
                    .any(|decision| decision.name == row.name && !decision.refusals.is_empty()));
        if refused {
            counts.not_run += 1;
            not_run += 1;
            continue;
        }
        if row.status == STATUSES[0] {
            counts.pass += 1;
            continue;
        }
        counts.fail += 1;
        let error: Option<&CodedText> =
            row.error.as_ref().filter(|error| !error.message.is_empty());
        let (mut entry, reason) = test_failure_entry(&TestFailureFacts {
            name: row.name.clone(),
            kind: SCENARIO_NOUNS.0.to_owned(),
            run_on: if row.local { "local" } else { "warehouse" }.to_owned(),
            error_code: error.and_then(|error| error.code.clone()),
            error_message: error.map(|error| error.message.clone()),
            error_help: error.and_then(|error| error.help.clone()),
            steps: scenario_steps(row),
            ..TestFailureFacts::default()
        });
        if let Some(path) = &row.local_duckdb_path {
            entry.fields.push(FailureField::Labelled {
                label: "duckdb".to_owned(),
                text: path.clone(),
            });
        }
        failed.push((entry.name.clone(), reason));
        entries.push(entry);
    }
    let columns: usize = terminal_columns(py)?;
    let mut output: String = String::new();
    if not_run > 0 {
        let all: Vec<HeaderCause> = refusal_causes(&report.decisions);
        let mut causes: Vec<String> = all
            .iter()
            .take(ROLLUP_CAUSES)
            .map(|cause| format!("{} {}", cause.label, cause.count))
            .collect();
        if all.len() > ROLLUP_CAUSES {
            causes.push(format!("+{} more", all.len() - ROLLUP_CAUSES));
        }
        output.push_str(&format!(
            "\n{}\n",
            run.style.muted(&format!(
                "  Can't run locally ({not_run}): {}; see below",
                causes.join(", ")
            ))
        ));
    }
    output.push_str(&render_failures(&entries, columns, run.style));
    output.push_str(&match report.mode {
        RunOn::Warehouse => String::new(),
        RunOn::Local => render_refusal_report(
            &report.decisions,
            (true, report.verbose),
            SCENARIO_NOUNS,
            (columns, run.style),
        ),
        RunOn::Auto if report.verbose => render_refusal_report(
            &report.decisions,
            (false, true),
            SCENARIO_NOUNS,
            (columns, run.style),
        ),
        RunOn::Auto => String::new(),
    });
    if flag(request, "retain")? {
        output.push_str(&retained_section(&rows, run.style));
    }
    output.push_str(&render_failed_list(&failed, report.verbose, run.style));
    let local: usize = results
        .iter()
        .filter(|result| result.local_status.is_some())
        .count();
    let split: (usize, usize) = (local, results.len() - local);
    let mut parts: Vec<String> = Vec::new();
    if split.0 - not_run.min(split.0) > 0 {
        parts.push(format!("{} local", split.0 - not_run.min(split.0)));
    }
    if split.1 > 0 {
        parts.push(format!("{} on the warehouse", split.1));
    }
    if not_run > 0 {
        parts.push(format!("{not_run} can't run locally"));
    }
    let mut details: Vec<String> = vec![parts.join(", ")];
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
        details.push(line);
    }
    if let Some(value) = &run.project.namespace.value {
        details.push(format!("namespace {value}"));
    }
    output.push_str(&render_run_completion(
        &counts,
        &details,
        elapsed_seconds,
        run.style,
    ));
    write(&run.stream, &output)?;
    flush(&run.stream)?;
    let succeeded: bool = counts.all_passed();
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

/// Run `scenarios` locally on run-scoped DuckDB or on the warehouse, as `run` is set up.
fn execute_batch<'py>(
    request: &Bound<'py, PyAny>,
    run: &TestRun<'py>,
    (pipeline, scenarios): (&Bound<'py, PyAny>, &Bound<'py, PyTuple>),
    (section, fallback, rows): (&SectionStatus<'py>, bool, &RowReporter<'py>),
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
            rows: Some((&rows.progress, rows.refused.clone(), rows.local_only)),
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
    local_attempt(LocalAttemptStep::HoldReplays(py, fallback))?;
    let ran: PyResult<Bound<'py, PyAny>> = call_with(&host_entry(py, runner)?, &arguments);
    local_attempt(LocalAttemptStep::ReleaseReplays(py))?;
    Vec::<ScenarioRunResult<_>>::read(&ran?)
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
    let planning: Bound<'_, PyAny> = planning_reporter(&run.stream, run.style)?;
    let base: Option<BaseComparison> = since_base(request, &run)?;
    let (pipeline, scenarios, since_json) = compiled(request, &run, &planning, base)?;
    run.since_json = since_json;
    if run.local && (flag(request, "sync_snapshots")? || flag(request, "refresh")?) {
        let synced: LocalSync<'_> = sync_local_snapshots(request, &run, (&pipeline, &scenarios))?;
        if synced.exit_code != 0 {
            return synced.write_failure_json(request, &run.project.namespace);
        }
    }
    let mut decisions: Vec<RunOnDecision> = if run.local {
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
    let header: RunHeader = run_header(&decisions, mode, flag(request, "explain")?);
    write(
        &run.stream,
        &format!(
            "\n{}{}",
            render_run_header(&header, terminal_columns(py)?, run.style),
            namespace_line(&run.project.namespace, run.style)
        ),
    )?;
    write_execution(
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
    let section: SectionStatus<'_> = quiet_section(&run.stream, run.style)?;
    let rows: RowReporter<'_> = RowReporter {
        progress: Bound::new(
            py,
            NestedCommandProgressCallbacks::new(
                py,
                Some(keywords(
                    py,
                    &[
                        (
                            "total",
                            int(py, i64::try_from(scenarios.len()).unwrap_or(i64::MAX)),
                        ),
                        ("label", text(py, "scenario")),
                        ("stream", run.stream.clone()),
                        ("use_color", boolean(py, run.style.use_color)),
                        ("name_width", int(py, name_width(&scenarios)?)),
                        (
                            "nouns",
                            (SCENARIO_NOUNS.0, SCENARIO_NOUNS.1)
                                .into_pyobject(py)?
                                .into_any(),
                        ),
                        ("verb", text(py, "Running scenarios")),
                        ("verbose", boolean(py, flag(request, "verbose")?)),
                    ],
                )?),
            )?,
        )?
        .into_any(),
        refused: decisions
            .iter()
            .filter(|decision| !decision.refusals.is_empty())
            .map(|decision| decision.name.clone())
            .collect(),
        local_only: mode == RunOn::Local,
    };
    let projector: Bound<'_, PyAny> = current_native_progress_projector(py)?;
    if !projector.is_none() {
        projector.call_method0("quiet_enriched_resources")?;
    }
    let started: Instant = Instant::now();
    let mut results: Vec<ScenarioRunResult<Bound<'_, PyAny>>> = Vec::new();
    let mut local_statements: StatementSummary = StatementSummary::default();
    if run.local {
        let before: StatementSummary = statement_summary(py)?;
        let ran = execute_batch(
            request,
            &run,
            (&pipeline, &named(&scenarios, &local_names)?),
            (&section, mode == RunOn::Auto, &rows),
        )?;
        results.extend(split_scenario_fallbacks(ran, (&mut decisions, mode))?);
        local_statements = statement_summary_delta(&before, &statement_summary(py)?);
    }
    let warehouse_names: Vec<&str> = decisions
        .iter()
        .filter(|decision| mode != RunOn::Local && !decision.local())
        .map(|decision| decision.name.as_str())
        .collect();
    let reporting: TestRun<'_> = if warehouse_names.is_empty() || !run.local {
        if !run.local {
            results.extend(execute_batch(
                request,
                &run,
                (&pipeline, &named(&scenarios, &warehouse_names)?),
                (&section, false, &rows),
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
            (&section, false, &rows),
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_scenario_test_command, module)?)?;
    Ok(())
}
