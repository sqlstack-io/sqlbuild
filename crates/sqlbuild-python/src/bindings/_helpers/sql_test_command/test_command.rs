//! `sqb test`: compile the selected SQL unit tests, inspect or run them, and report; locally, then run the Python tests.

use sqlbuild_python_base::native_host::models::Dialect;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use crate::bindings::_helpers::scenario_command::presentation::child_records;
use pyo3::IntoPyObject;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyInt, PyString, PyTuple};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_analysis::definition_changes::models::{ExecutedItem, SelectedItem};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::progress::main::statement_summary_delta::statement_summary_delta;
use sqlbuild_core::progress::models::ExpectationStep;
use sqlbuild_core::progress::models::StatementSummary;
use sqlbuild_core::run_on::main::apply_local_refusals::apply_local_refusals;
use sqlbuild_core::run_on::main::execution_engines::execution_engines;
use sqlbuild_core::run_on::main::read_run_on_decisions::read_run_on_decisions;
use sqlbuild_core::run_on::main::refusal_causes::refusal_causes;
use sqlbuild_core::run_on::main::relied_on_text::relied_on_text;
use sqlbuild_core::run_on::main::render_refusal_report::render_refusal_report;
use sqlbuild_core::run_on::main::run_on_header::run_on_header;
use sqlbuild_core::run_on::main::with_run_on_json::with_run_on_json;
use sqlbuild_core::run_on::models::{LocalRefusal, RunOn, RunOnDecision};
use sqlbuild_core::run_output::main::render_cant_run_rollup::render_cant_run_rollup;
use sqlbuild_core::run_output::main::render_failed_list::render_failed_list;
use sqlbuild_core::run_output::main::render_failures::render_failures;
use sqlbuild_core::run_output::main::render_moved_note::render_moved_note;
use sqlbuild_core::run_output::main::render_run_completion::render_run_completion;
use sqlbuild_core::run_output::main::render_run_header::render_run_header;
use sqlbuild_core::run_output::models::{FailureEntry, HeaderCause, RunCounts};
use sqlbuild_core::test_command::main::expectation_child_rows::expectation_child_rows;
use sqlbuild_core::test_command::main::render_test_inspection::render_test_inspection;
use sqlbuild_core::test_command::main::test_exit_code::test_exit_code;
use sqlbuild_core::test_command::main::test_failure_entry::test_failure_entry;
use sqlbuild_core::test_command::main::test_group_name::test_group_name;
use sqlbuild_core::test_command::main::test_name_width::test_name_width;
use sqlbuild_core::test_command::main::test_outcome_status::test_outcome_status;
use sqlbuild_core::test_command::models::{TestFailureFacts, TestInspectionEntry, TestPlanWarning};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::{
    SqlTestColumnDifference, SqlTestDifferenceSample, SqlTestExecutionResult, SqlTestOutcome,
};
use sqlbuild_host::types::ResultEnum;
use sqlbuild_orchestration::runs::models::NodeOutcome;
use sqlbuild_python_base::build_progress::main::terminal::terminal_columns::terminal_columns;
use sqlbuild_python_base::build_progress::models::NativeProgressProjector;

use crate::bindings::_helpers::change_selection::base_snapshot::BaseComparison;
use crate::bindings::_helpers::change_selection::base_snapshot::base_comparison;
use crate::bindings::_helpers::change_selection::base_snapshot::base_variant;
use crate::bindings::_helpers::change_selection::selection::ChangedProject;
use crate::bindings::_helpers::change_selection::selection::changed_project;
use crate::bindings::_helpers::change_selection::selection::tests_and_models;
use crate::bindings::_helpers::change_selection::selection::with_since_section;
use crate::bindings::_helpers::command_runtime::invocation::resolve_reported_command_invocation;
use crate::bindings::_helpers::compile_driver::static_context::compile_static_command_context;
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use crate::bindings::_helpers::native_host::statement_report::StatementReport;
use crate::bindings::_helpers::sql_test_command::python_tests::PYTHON_TESTS_INPUT_CODE;
use crate::bindings::_helpers::sql_test_command::python_tests::names_python_node;
use crate::bindings::_helpers::sql_test_command::python_tests::python_test_modules;
use crate::bindings::_helpers::sql_test_command::python_tests::run_python_tests;
use crate::bindings::_helpers::sql_test_command::python_tests::selected_python_tests;
use crate::bindings::_helpers::sql_test_command::python_tests::selective;
use crate::bindings::_helpers::sql_test_command::python_tests::with_python_section;
use crate::bindings::_helpers::sql_test_command::timeouts::RunTimeout;
use crate::bindings::_helpers::sql_test_command::timeouts::TimeoutKind;
use crate::bindings::_helpers::sql_test_command::timeouts::resolve_run_timeout;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_command::main::emit::emit;
use sqlbuild_python_base::build_progress::main::progress::current_native_progress_projector::current_native_progress_projector;
use sqlbuild_python_base::build_progress::main::progress::current_projector::current_projector;
use sqlbuild_python_base::build_progress::main::transient_status::status_reporter::status_reporter;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::types::ReadResult;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_compile::compile_driver::main::setup::build_static_pipeline_result::build_static_pipeline_result;
use sqlbuild_python_compile::compile_outputs::main::planned_sql_test_path::planned_sql_test_path;
use sqlbuild_python_compile::compile_outputs::main::write_test_runtime_target::write_test_runtime_target;
use sqlbuild_python_front::command_runtime::main::resolve_concurrency::resolve_concurrency;
use sqlbuild_python_front::command_runtime::main::resolve_run_on::resolve_run_on;
use sqlbuild_python_output::build_progress::main::local_attempt::local_attempt;
use sqlbuild_python_output::build_progress::main::test_label::test_label;
use sqlbuild_python_output::build_progress::main::test_result_label::test_result_label;
use sqlbuild_python_output::build_progress::main::write_execution_header::write_execution_header;
use sqlbuild_python_output::build_progress::models::ConnectionProgressReporter;
use sqlbuild_python_output::build_progress::models::LocalAttemptStep;
use sqlbuild_python_output::build_progress::models::NestedCommandProgressCallbacks;
use sqlbuild_python_output::result_output::main::events::sql_test_identity::sql_test_identity;
use sqlbuild_python_output::result_output::main::results::local_refusal::local_refusal;
use sqlbuild_python_output::result_output::main::results::test_execution_json::test_execution_json;
use sqlbuild_python_output::result_output::main::results::write_execution_json_output::write_execution_json_output;
use sqlbuild_python_output::result_output::models::ExecutionEventWriter;
use sqlbuild_python_planner::planner_entries::main::plans::build_test_command_plan::build_test_command_plan;
use sqlbuild_python_project::project_snapshot::models::WorkerBuild;
use sqlbuild_python_project::project_snapshot::types::VerifiedRun;

/// Why `--since` selected each test, by inspection label.
type SelectionReasons = HashMap<String, Vec<String>>;
/// What a `--since` selection kept: the narrowed pipeline, reasons by label, and its JSON section.
type ChangedSelection<'py> = (Bound<'py, PyAny>, SelectionReasons, String);
/// The error of a test whose expectations failed; its child rows already say which.
const EXPECTATIONS_FAILED_CODE: &str = "T003";
const UNKNOWN_GROUP: &str = "(unknown)";
const EXECUTION_COMMAND: &str = "sqb test";
const UNKNOWN_CASE_CODE: &str = "S008";
const RUN_ON_SETTING: &str = "test_run_on";
const TEST_NOUNS: (&str, &str) = ("test", "tests");
const LOCAL_RUN_ON: &str = "local";
const WAREHOUSE_RUN_ON: &str = "warehouse";
const CANNOT_RUN_LOCALLY_CODE: &str = "T007";
const CANNOT_RUN_LOCALLY_MESSAGE: &str = "can't run locally";
const NEEDS_WAREHOUSE_CODE: &str = "T008";
const NEEDS_WAREHOUSE_MESSAGE: &str = "needs the warehouse (no connection)";
const COMPILED_TESTS_DIR: &str = "target/run/tests";
/// The child row name of the expected rows of the model a test is grouped under.
const JSON_CHECK_NAME_KEYS: [&str; 3] = ["test_name", "name", "resource_name"];

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn strings(values: Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

/// The widest name a test row shows: without its group's prefix when the group has a header.
fn name_width(entries: &Bound<'_, PyAny>) -> PyResult<usize> {
    let counts: HashMap<String, usize> = group_counts(entries)?;
    let mut labels: Vec<(String, String)> = Vec::new();
    let mut children: Vec<String> = Vec::new();
    for entry in entries.try_iter()? {
        let entry: Bound<'_, PyAny> = entry?;
        for step in entry.getattr("chain")?.try_iter()? {
            let step: Bound<'_, PyAny> = step?;
            if !step.getattr("expected_cte_sql")?.is_none() {
                children.push(step.getattr("model_name")?.extract()?);
            }
        }
        for assertion in entry.getattr("assertions")?.try_iter()? {
            children.push(assertion?.getattr("name")?.extract()?);
        }
        let group: String = entry_group(&entry)?;
        let headed: bool = counts.get(&group).copied().unwrap_or(0) > 1;
        labels.push((
            test_label(&entry, "name")?,
            if headed { group } else { String::new() },
        ));
    }
    Ok(test_name_width(&labels, &children))
}

/// How many selected tests each group header heads.
fn group_counts(entries: &Bound<'_, PyAny>) -> PyResult<HashMap<String, usize>> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for entry in entries.try_iter()? {
        *counts.entry(entry_group(&entry?)?).or_default() += 1;
    }
    Ok(counts)
}

fn outcome_status(outcome: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    Ok(test_outcome_status(
        test_outcome(outcome)?.map_or("", ResultEnum::value),
    ))
}

/// The typed outcome of a test or step outcome member; `None` for a value this build does not know.
fn test_outcome(outcome: &Bound<'_, PyAny>) -> PyResult<Option<SqlTestOutcome>> {
    Ok(SqlTestOutcome::from_value(
        &outcome.getattr("value")?.extract::<String>()?,
    ))
}

/// One child row per step: `expected rows`, `expected <model>` for another model, or `assertion <name>`.
fn expectation_rows<'py>(result: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    child_records(
        result.py(),
        &expectation_child_rows(&expectation_steps(&SqlTestExecutionResult::read(result)?)),
    )
}

/// `N real models` for a model test, else empty.
fn real_model_detail(result: &Bound<'_, PyAny>) -> PyResult<String> {
    let count: Option<usize> = native_field(result, "real_model_count")?.extract()?;
    Ok(match count {
        Some(1) => "1 real model".to_owned(),
        Some(count) => format!("{count} real models"),
        None => String::new(),
    })
}

/// The model a planned test's progress rows are grouped under.
fn entry_group(entry: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut chain: Vec<(String, bool)> = Vec::new();
    for step in entry.getattr("chain")?.try_iter()? {
        let step: Bound<'_, PyAny> = step?;
        chain.push((
            step.getattr("model_name")?.extract()?,
            !step.getattr("expected_cte_sql")?.is_none(),
        ));
    }
    Ok(test_group_name(&chain))
}

/// Forward test progress, completions and connection timing to their reporters.
#[pyclass(module = "sqlbuild._native", frozen)]
struct TestCallbackRelay {
    worker: Option<Arc<WorkerBuild>>,
    /// Where each test runs, by test name; a local test that falls back becomes `warehouse`.
    run_on: Mutex<HashMap<String, String>>,
    /// What each local test relied on from the project, by test name, for its row.
    relied_on: HashMap<String, String>,
    /// In `auto`, a local test DuckDB can't run as the warehouse would runs there instead, unreported.
    fallback: bool,
    progress: Py<PyAny>,
    event_writer: Py<PyAny>,
    plan_output: Py<PyAny>,
    connection: Py<PyAny>,
}

#[pymethods]
impl TestCallbackRelay {
    fn on_test_start(&self, py: Python<'_>, entry: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(worker) = &self.worker {
            worker.start_attached(py, &sql_test_identity(entry, "name")?)?;
        }
        call_with(
            &self.progress.bind(py).getattr("on_item_start")?,
            &[
                ("group_name", text(py, &entry_group(entry)?)),
                ("item_name", text(py, &test_label(entry, "name")?)),
                ("canonical_resource_name", entry.getattr("name")?),
            ],
        )?;
        Ok(())
    }

    fn on_test_finished(&self, py: Python<'_>, entry: &Bound<'_, PyAny>) -> PyResult<()> {
        self.progress
            .bind(py)
            .call_method1("on_item_finished", (test_label(entry, "name")?,))?;
        Ok(())
    }

    fn on_test_complete(&self, py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
        if let (true, Some((name, refusal))) = (self.fallback, refused(result)?) {
            let mut run_on = self.run_on.lock().unwrap_or_else(PoisonError::into_inner);
            if run_on.get(&name).map(String::as_str) == Some(LOCAL_RUN_ON) {
                run_on.insert(name, format!("{WAREHOUSE_RUN_ON}  ({})", refusal.title()));
                let step = LocalAttemptStep::Abandon(self.progress.bind(py), result);
                return local_attempt(step).map(drop);
            }
        }
        let mut group: String = String::new();
        if let Some(step) = result.getattr("step_results")?.try_iter()?.next() {
            group = step?.getattr("model_name")?.extract()?;
        }
        if group.is_empty() {
            group = UNKNOWN_GROUP.to_owned();
        }
        let identity: String = sql_test_identity(result, "test_name")?;
        let outcome: Bound<'_, PyAny> = result.getattr("outcome")?;
        let status: &str = outcome_status(&outcome)?;
        if let Some(worker) = &self.worker {
            let outcome = if test_outcome(&outcome)? == Some(SqlTestOutcome::Pass) {
                NodeOutcome::Success
            } else {
                NodeOutcome::Failed
            };
            worker.finish_attached(py, &identity, outcome)?;
        }
        let resource_id: Bound<'_, PyAny> = text(py, &identity);
        let error_code: Option<String> = result.getattr("error_code")?.extract()?;
        let not_run: bool = matches!(
            error_code.as_deref(),
            Some(CANNOT_RUN_LOCALLY_CODE | NEEDS_WAREHOUSE_CODE)
        );
        let children: Bound<'_, PyTuple> = expectation_rows(result)?;
        let error = |name: &str| -> PyResult<Bound<'_, PyAny>> {
            if error_code.as_deref() == Some(EXPECTATIONS_FAILED_CODE) && children.len()? > 0 {
                return Ok(py.None().into_bound(py));
            }
            result.getattr(name)
        };
        call_with(
            &self.progress.bind(py).getattr("on_item_complete")?,
            &[
                ("group_name", text(py, &group)),
                ("item_name", text(py, &test_label(result, "test_name")?)),
                ("status_text", text(py, status)),
                ("child_rows", children.clone().into_any()),
                ("run_on", text(py, &self.run_on(result)?)),
                ("detail", text(py, &self.detail(result)?)),
                ("not_run", PyBool::new(py, not_run).to_owned().into_any()),
                ("error_code", error("error_code")?),
                ("error_help", error("error_help")?),
                ("error_message", error("error_message")?),
                ("canonical_resource_name", result.getattr("test_name")?),
                ("canonical_resource_id", resource_id),
            ],
        )?;
        call_with(
            &self.event_writer.bind(py).getattr("write_build_result")?,
            &[
                ("result", result.clone()),
                ("plan", self.plan_output.bind(py).clone()),
                ("command", text(py, "test")),
            ],
        )?;
        Ok(())
    }

    fn on_connection_complete(
        &self,
        py: Python<'_>,
        connection_count: i64,
        elapsed_seconds: f64,
    ) -> PyResult<()> {
        self.connection_call(
            py,
            "on_connection_complete",
            (connection_count, elapsed_seconds),
        )
    }

    fn on_connection_error(
        &self,
        py: Python<'_>,
        connection_count: i64,
        elapsed_seconds: f64,
    ) -> PyResult<()> {
        self.connection_call(
            py,
            "on_connection_error",
            (connection_count, elapsed_seconds),
        )
    }
}

impl TestCallbackRelay {
    /// What a local test relied on from the project, then `N real models`.
    fn detail(&self, result: &Bound<'_, PyAny>) -> PyResult<String> {
        let name: String = result.getattr("test_name")?.extract()?;
        let real: String = real_model_detail(result)?;
        let run_on = self.run_on.lock().unwrap_or_else(PoisonError::into_inner);
        let relied: Option<&String> = match run_on.get(&name) {
            Some(placed) if placed == LOCAL_RUN_ON => self.relied_on.get(&name),
            _ => None,
        };
        Ok(match relied {
            Some(relied) if real.is_empty() => relied.clone(),
            Some(relied) => format!("{relied}  {real}"),
            None => real,
        })
    }

    /// Where the test ran: `local` or `warehouse`; empty when the run did not decide.
    fn run_on(&self, result: &Bound<'_, PyAny>) -> PyResult<String> {
        let name: String = result.getattr("test_name")?.extract()?;
        let run_on = self.run_on.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(run_on.get(&name).map_or_else(String::new, |run_on| {
            run_on.split("  ").next().unwrap_or_default().to_owned()
        }))
    }

    fn connection_call(
        &self,
        py: Python<'_>,
        method: &str,
        (connection_count, elapsed_seconds): (i64, f64),
    ) -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("connection_count", connection_count)?;
        options.set_item("elapsed_seconds", elapsed_seconds)?;
        self.connection
            .bind(py)
            .call_method(method, (), Some(&options))?;
        Ok(())
    }
}

/// The test that ran locally and failed where the warehouse would not, and why, when it did.
fn refused(result: &Bound<'_, PyAny>) -> PyResult<Option<(String, LocalRefusal)>> {
    local_refusal(&native_field(result, "local_refusal")?)
}

fn unknown_case(py: Python<'_>, message: String) -> PyErr {
    new_error(
        py,
        ErrorId::PlannerInput,
        &message,
        (Some(UNKNOWN_CASE_CODE), None),
    )
}

/// Compile the selected tests and refuse a `--case` that matched none of them.
fn compile_tests<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Bound<'py, PyAny>,
    python_selectors: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let discovered: Bound<'py, PyAny> = invocation.getattr("discovered_inputs")?;
    let adapter: Bound<'py, PyAny> = invocation.getattr("adapter")?;
    let resolver: Bound<'py, PyAny> = resolve_external_sql_reference_resolver(
        invocation.getattr("effective_project_dir")?,
        discovered.clone(),
    )?;
    let options: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CompilePipelineOptions,
        &[
            ("selected_target", request.getattr("selected_target")?),
            ("no_sql_validation", request.getattr("no_sql_validation")?),
            (
                "source_deferral_enabled",
                PyBool::new(py, false).to_owned().into_any(),
            ),
            ("select", request.getattr("select")?),
            ("exclude", request.getattr("exclude")?),
            (
                "accepts_unit_test_selectors",
                PyBool::new(py, true).to_owned().into_any(),
            ),
            (
                "resolve_python_run_selectors",
                PyBool::new(py, python_selectors).to_owned().into_any(),
            ),
            (
                "connection_config",
                invocation.getattr("connection_config")?,
            ),
            ("cli_vars", request.getattr("cli_vars")?),
            ("external_sql_reference_resolver", resolver),
        ],
    )?;
    let context: Bound<'py, PyAny> = compile_static_command_context(Some(&keywords(
        py,
        &[
            ("discovered_inputs", discovered),
            ("adapter", adapter.clone()),
            ("options", options),
            (
                "on_progress",
                native_field(
                    &native_field(invocation, "planning_progress")?,
                    "on_progress",
                )?,
            ),
        ],
    )?))?;
    let case_name: Bound<'py, PyAny> = request.getattr("case_name")?;
    let plan_output: Bound<'py, PyAny> = build_test_command_plan(
        py,
        Some(keywords(
            py,
            &[
                ("project", context.getattr("project")?),
                ("dialect", Dialect::of(&adapter)?.into_any()),
                ("scope", context.getattr("scope")?),
                ("relations", context.getattr("relations")?),
                ("case_name", case_name.clone()),
            ],
        )?),
    )?;
    if !case_name.is_none() && !plan_output.getattr("test_entries")?.is_truthy()? {
        let cases: Vec<String> = strings(plan_output.getattr("available_test_case_names")?)?;
        let available: String = if cases.is_empty() {
            "none".to_owned()
        } else {
            cases.join(", ")
        };
        return Err(unknown_case(
            py,
            format!(
                "SQL test case '{}' did not match the selected tests; available cases: {available}",
                case_name.str()?
            ),
        ));
    }
    build_static_pipeline_result(
        py,
        Some(keywords(
            py,
            &[("context", context), ("plan_output", plan_output)],
        )?),
    )
}

fn inspection_entry(entry: &Bound<'_, PyAny>) -> PyResult<TestInspectionEntry> {
    let case: Bound<'_, PyAny> = entry.getattr("case_name")?;
    let name: String = entry.getattr("name")?.extract()?;
    let label: String = if case.is_none() {
        name
    } else {
        let parent: Bound<'_, PyAny> = entry.getattr("parent_name")?;
        let parent: String = if parent.is_truthy()? {
            parent.extract()?
        } else {
            name
        };
        format!("{parent} [{}]", case.str()?)
    };
    let mut real_models: Vec<String> = Vec::new();
    let mut expected_models: Vec<String> = Vec::new();
    for step in entry.getattr("chain")?.try_iter()? {
        let step: Bound<'_, PyAny> = step?;
        let model: String = step.getattr("model_name")?.extract()?;
        if !step.getattr("expected_cte_sql")?.is_none() {
            expected_models.push(model.clone());
        }
        real_models.push(model);
    }
    Ok(TestInspectionEntry {
        label,
        mocked_sources: strings(entry.getattr("mock_source_names")?)?,
        mocked_refs: strings(entry.getattr("mock_ref_names")?)?,
        mocked_seeds: strings(entry.getattr("mock_seed_names")?)?,
        mocked_dbt_refs: strings(entry.getattr("mock_dbt_ref_names")?)?,
        mocked_table_functions: strings(entry.getattr("mock_table_function_names")?)?,
        real_models,
        expected_models,
        selection_reasons: Vec::new(),
        run_on: None,
    })
}

/// Write the resolved plan without executing it; returns the exit code.
fn inspect(
    stream: &Bound<'_, PyAny>,
    plan_output: &Bound<'_, PyAny>,
    (reasons, decisions): (&HashMap<String, Vec<String>>, &[RunOnDecision]),
) -> PyResult<i32> {
    let by_name: HashMap<&str, &RunOnDecision> = decisions
        .iter()
        .map(|decision| (decision.name.as_str(), decision))
        .collect();
    let entries: Vec<TestInspectionEntry> = native_field(plan_output, "test_entries")?
        .try_iter()?
        .map(|entry| {
            let entry: Bound<'_, PyAny> = entry?;
            let name: String = entry.getattr("name")?.extract()?;
            let mut entry: TestInspectionEntry = inspection_entry(&entry)?;
            entry.selection_reasons = reasons.get(&entry.label).cloned().unwrap_or_default();
            entry.run_on = by_name.get(name.as_str()).copied().cloned();
            Ok(entry)
        })
        .collect::<PyResult<_>>()?;
    let warnings: Vec<TestPlanWarning> = native_field(plan_output, "warnings")?
        .try_iter()?
        .map(|warning| {
            let warning: Bound<'_, PyAny> = warning?;
            Ok(TestPlanWarning {
                severity: warning.getattr("severity")?.getattr("value")?.extract()?,
                message: warning.getattr("message")?.str()?.extract()?,
            })
        })
        .collect::<PyResult<_>>()?;
    let (rendered, code) = render_test_inspection(&entries, &warnings);
    stream.call_method1("write", (rendered,))?;
    stream.call_method0("flush")?;
    Ok(code)
}

/// Keep the planned tests that execute something changed, with each one's reasons by label.
fn select_changed<'py>(
    (pipeline, stream, discovered): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    project_dir: &std::path::Path,
    base: BaseComparison,
    explain: bool,
) -> PyResult<ChangedSelection<'py>> {
    let py: Python<'py> = pipeline.py();
    let project: Bound<'py, PyAny> = native_field(pipeline, "project")?;
    let changed: ChangedProject = changed_project((&project, discovered), project_dir, base)?;
    let (tests, models) = tests_and_models(&project)?;
    let plan_output: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    let mut kept: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut selected: Vec<SelectedItem> = Vec::new();
    let mut reasons: HashMap<String, Vec<String>> = HashMap::new();
    let mut total: usize = 0;
    for entry in native_field(&plan_output, "test_entries")?.try_iter()? {
        let entry: Bound<'py, PyAny> = entry?;
        total += 1;
        let item: ExecutedItem = changed.test_item(&entry, (&tests, &models))?;
        let entry_reasons: Vec<String> = changed.reasons(&item);
        if entry_reasons.is_empty() {
            continue;
        }
        let label: String = inspection_entry(&entry)?.label;
        reasons.insert(label.clone(), entry_reasons.clone());
        selected.push(SelectedItem {
            label,
            reasons: entry_reasons,
        });
        kept.push(entry);
    }
    changed.report(stream, ("test", "tests"), (total, &selected, explain))?;
    let plan_output: Bound<'py, PyAny> = replaced(
        &plan_output,
        &[("test_entries", PyTuple::new(py, kept)?.into_any())],
    )?;
    Ok((
        replaced(pipeline, &[("plan_output", plan_output)])?,
        reasons,
        changed.json((total, &selected)),
    ))
}

/// Where each selected test goes: an error row, the local DuckDB batch, or the warehouse batch.
pub(crate) struct TestPlacement<'py> {
    /// Tests that do not run, with the code and message of their error row.
    unplaced: Vec<(Bound<'py, PyAny>, &'static str, &'static str)>,
    local: Vec<Bound<'py, PyAny>>,
    /// The local tests' names, in the same order.
    local_names: Vec<String>,
    warehouse: Vec<Bound<'py, PyAny>>,
    /// Where each test runs, by name, for its row.
    run_on: HashMap<String, String>,
    /// What each local test relied on from the project, by name, for its row.
    relied_on: HashMap<String, String>,
    /// In `auto`, warehouse tests without a connection become error rows instead of failing the run.
    warehouse_optional: bool,
}

/// Results in plan order, local statements, and each local test failing where the warehouse wouldn't.
type RanTests<'py> = (
    Vec<SqlTestExecutionResult<Bound<'py, PyAny>>>,
    StatementSummary,
    Vec<(String, LocalRefusal)>,
);

/// Run the tests with nested progress, local ones first; in `auto`, those that fall back after.
fn run_tests<'py>(
    invocation: &Bound<'py, PyAny>,
    pipeline: &Bound<'py, PyAny>,
    ((concurrency, timeout), worker, verbose): ((i64, &RunTimeout), Option<Arc<WorkerBuild>>, bool),
    placement: TestPlacement<'py>,
) -> PyResult<RanTests<'py>> {
    let py: Python<'py> = invocation.py();
    let stream: Bound<'py, PyAny> = invocation.getattr("progress_stream")?;
    let use_color: Bound<'py, PyAny> = invocation.getattr("use_color")?;
    let plan_output: Bound<'py, PyAny> = pipeline.getattr("plan_output")?;
    let entries: Bound<'py, PyAny> = plan_output.getattr("test_entries")?;
    let test_count: usize = entries.len()?;
    let progress: Bound<'py, PyAny> = Bound::new(
        py,
        NestedCommandProgressCallbacks::new(
            py,
            Some(keywords(
                py,
                &[
                    ("total", PyInt::new(py, test_count).into_any()),
                    ("label", text(py, "test")),
                    ("stream", stream.clone()),
                    ("use_color", use_color.clone()),
                    (
                        "name_width",
                        PyInt::new(py, name_width(&entries)?).into_any(),
                    ),
                    (
                        "group_counts",
                        group_counts(&entries)?.into_pyobject(py)?.into_any(),
                    ),
                    ("verbose", PyBool::new(py, verbose).to_owned().into_any()),
                ],
            )?),
        )?,
    )?
    .into_any();
    let projector: Bound<'py, PyAny> = current_native_progress_projector(py)?;
    if !projector.is_none() {
        projector.call_method0("quiet_enriched_resources")?;
        for entry in entries.try_iter()? {
            call_with(
                &projector.getattr("expect_resource_enrichment")?,
                &[("resource_name", entry?.getattr("name")?)],
            )?;
        }
    }
    let connection: Bound<'py, PyAny> = Bound::new(
        py,
        ConnectionProgressReporter::new(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter_name", invocation.getattr("adapter_name")?),
                    ("stream", stream.clone()),
                    ("use_color", use_color.clone()),
                ],
            )?),
        )?,
    )?
    .into_any();
    let preflight: Bound<'py, PyAny> = status_reporter(&stream, use_color.is_truthy()?)?;
    let event_writer: Bound<'py, PyAny> =
        Bound::new(py, ExecutionEventWriter::new(py, None)?)?.into_any();
    let TestPlacement {
        unplaced,
        local,
        local_names,
        mut warehouse,
        run_on,
        relied_on,
        warehouse_optional,
    } = placement;
    let mut refused_locally: Vec<(String, LocalRefusal)> = Vec::new();
    let relay: Bound<'py, TestCallbackRelay> = Bound::new(
        py,
        TestCallbackRelay {
            worker,
            run_on: Mutex::new(run_on),
            relied_on,
            fallback: warehouse_optional,
            progress: progress.unbind(),
            event_writer: event_writer.clone().unbind(),
            plan_output: plan_output.clone().unbind(),
            connection: connection.clone().unbind(),
        },
    )?;
    let mut local_statements: StatementSummary = StatementSummary::default();
    let executed: PyResult<Vec<Bound<'py, PyAny>>> = (|| {
        let mut results: Vec<Bound<'py, PyAny>> = Vec::new();
        for (code, message) in [
            (CANNOT_RUN_LOCALLY_CODE, CANNOT_RUN_LOCALLY_MESSAGE),
            (NEEDS_WAREHOUSE_CODE, NEEDS_WAREHOUSE_MESSAGE),
        ] {
            let entries: Vec<Bound<'py, PyAny>> = unplaced
                .iter()
                .filter(|(_, unplaced_code, _)| *unplaced_code == code)
                .map(|(entry, _, _)| entry.clone())
                .collect();
            results.extend(report_unplaced(
                &relay,
                pipeline,
                &entries,
                (code, message),
            )?);
        }
        let callbacks = |reporting_connection: bool| -> PyResult<Bound<'py, PyAny>> {
            let mut hooks: Vec<(&str, Bound<'py, PyAny>)> = vec![
                (
                    "on_progress",
                    preflight.getattr("report_preflight_progress")?,
                ),
                ("on_test_start", relay.getattr("on_test_start")?),
                ("on_test_finished", relay.getattr("on_test_finished")?),
                ("on_test_complete", relay.getattr("on_test_complete")?),
            ];
            if reporting_connection {
                hooks.extend([
                    (
                        "on_connection_start",
                        connection.getattr("on_connection_start")?,
                    ),
                    (
                        "on_connection_complete",
                        relay.getattr("on_connection_complete")?,
                    ),
                    ("on_connection_error", relay.getattr("on_connection_error")?),
                ]);
            }
            call_with(
                &host_entry(py, ExecutorHost::TestPipelineCallbacks)?,
                &hooks,
            )
        };
        let run = |batch: &[Bound<'py, PyAny>],
                   (adapter, connection_config): (Bound<'py, PyAny>, Bound<'py, PyAny>),
                   reporting_connection: bool|
         -> PyResult<Vec<Bound<'py, PyAny>>> {
            call_with(
                &host_entry(py, ExecutorHost::RunTestPipeline)?,
                &[
                    (
                        "plan",
                        replaced(
                            &plan_output,
                            &[("test_entries", PyTuple::new(py, batch)?.into_any())],
                        )?,
                    ),
                    ("connection_config", connection_config),
                    ("adapter", adapter),
                    ("max_concurrency", PyInt::new(py, concurrency).into_any()),
                    ("callbacks", callbacks(reporting_connection)?),
                    ("run_id", pipeline.getattr("project")?.getattr("run_id")?),
                    ("timeouts", timeout_record(py, timeout)?),
                ],
            )?
            .try_iter()?
            .collect()
        };
        if !local.is_empty() {
            let before: StatementSummary = statement_summary(py)?;
            let adapter: Bound<'py, PyAny> = call_with(
                &host_entry(py, ExecutorHost::LocalTestAdapter)?,
                &[
                    ("project", native_field(pipeline, "project")?),
                    ("adapter", invocation.getattr("adapter")?),
                ],
            )?;
            let memory: Bound<'py, PyDict> = PyDict::new(py);
            memory.set_item("database", ":memory:")?;
            let progress: &Bound<'py, PyAny> = relay.get().progress.bind(py);
            local_attempt(LocalAttemptStep::Hold(progress, warehouse_optional))?;
            let ran: PyResult<Vec<Bound<'py, PyAny>>> =
                run(&local, (adapter, memory.into_any()), false);
            local_attempt(LocalAttemptStep::Release(progress))?;
            for result in ran? {
                let Some((name, refusal)) = refused(&result)? else {
                    results.push(result);
                    continue;
                };
                let entry = local_names
                    .iter()
                    .position(|test| *test == name)
                    .map(|at| &local[at]);
                refused_locally.push((name, refusal));
                match (warehouse_optional, entry) {
                    (true, Some(entry)) => warehouse.push(entry.clone()),
                    _ => results.push(result),
                }
            }
            local_statements = statement_summary_delta(&before, &statement_summary(py)?);
        }
        if !warehouse.is_empty() {
            match run(
                &warehouse,
                (
                    invocation.getattr("adapter")?,
                    invocation.getattr("connection_config")?,
                ),
                true,
            ) {
                Ok(ran) => results.extend(ran),
                Err(error)
                    if warehouse_optional
                        && error.is_instance(
                            py,
                            &host_entry(py, ExecutorHost::WarehouseConnectionError)?,
                        ) =>
                {
                    results.extend(report_unplaced(
                        &relay,
                        pipeline,
                        &warehouse,
                        (NEEDS_WAREHOUSE_CODE, NEEDS_WAREHOUSE_MESSAGE),
                    )?);
                }
                Err(error) => return Err(error),
            }
        }
        Ok(results)
    })();
    let closed: PyResult<Bound<'py, PyAny>> = event_writer.call_method0("close");
    let ran: Vec<Bound<'py, PyAny>> = executed?;
    closed?;
    let mut by_name: HashMap<String, Bound<'py, PyAny>> = HashMap::new();
    for result in ran {
        by_name.insert(result.getattr("test_name")?.extract()?, result);
    }
    let mut results: Vec<SqlTestExecutionResult<Bound<'py, PyAny>>> = Vec::new();
    for entry in entries.try_iter()? {
        let name: String = entry?.getattr("name")?.extract()?;
        if let Some(result) = by_name.remove(&name) {
            results.push(SqlTestExecutionResult::read(&result)?);
        }
    }
    for result in by_name.into_values() {
        results.push(SqlTestExecutionResult::read(&result)?);
    }
    Ok((results, local_statements, refused_locally))
}

/// Error rows for tests that do not run, reported as the run reports every other test.
fn report_unplaced<'py>(
    relay: &Bound<'py, TestCallbackRelay>,
    pipeline: &Bound<'py, PyAny>,
    entries: &[Bound<'py, PyAny>],
    (code, message): (&str, &str),
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = relay.py();
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    call_with(
        &host_entry(py, ExecutorHost::ReportUnplacedSqlTests)?,
        &[
            ("test_entries", PyTuple::new(py, entries)?.into_any()),
            ("code", text(py, code)),
            ("message", text(py, message)),
            ("run_id", pipeline.getattr("project")?.getattr("run_id")?),
            ("on_test_start", relay.getattr("on_test_start")?),
            ("on_test_complete", relay.getattr("on_test_complete")?),
        ],
    )?
    .try_iter()?
    .collect()
}

/// The statements the installed projector has counted so far.
fn statement_summary(py: Python<'_>) -> PyResult<StatementSummary> {
    match current_projector(py)? {
        Some(projector) => projector.get().statement_summary(),
        None => Ok(StatementSummary::default()),
    }
}

/// The executor's `RunItemTimeouts` for a resolved run limit.
pub(crate) fn timeout_record<'py>(
    py: Python<'py>,
    timeout: &RunTimeout,
) -> PyResult<Bound<'py, PyAny>> {
    host_entry(py, ExecutorHost::RunItemTimeouts)?.call((), Some(&timeout.keywords(py)?))
}

/// Where the run sent its tests, for the summary, the refusal report and `--json`.
struct RunOnReport {
    mode: RunOn,
    decisions: Vec<RunOnDecision>,
    local_statements: StatementSummary,
    verbose: bool,
    /// Each test's compiled SQL under `target/run/tests/`, by name.
    compiled_paths: HashMap<String, String>,
    /// The tests the header already said can't run locally, before any ran.
    header_refused: HashSet<String>,
}

/// A differing column as its name, actual and expected value.
fn column_difference(difference: &SqlTestColumnDifference) -> (String, String, String) {
    (
        difference.name.clone(),
        difference.actual.clone(),
        difference.expected.clone(),
    )
}

/// The cells of each differing row sample.
fn sample_values(samples: &[SqlTestDifferenceSample]) -> Vec<Vec<(String, String)>> {
    samples.iter().map(|sample| sample.values.clone()).collect()
}

/// Each step of a result as the failure detail reads it.
fn expectation_steps(result: &SqlTestExecutionResult<Bound<'_, PyAny>>) -> Vec<ExpectationStep> {
    result
        .step_results
        .iter()
        .map(|step| ExpectationStep {
            model_name: step.model_name.clone(),
            outcome: step.outcome.value().to_owned(),
            actual_row_count: step.actual_row_count,
            expected_row_count: step.expected_row_count,
            mismatched_row_count: step.mismatched_row_count,
            unexpected_row_count: step.unexpected_row_count,
            missing_row_count: step.missing_row_count,
            unexpected_samples: sample_values(&step.unexpected_samples),
            missing_samples: sample_values(&step.missing_samples),
            column_differences: step
                .column_differences
                .iter()
                .map(column_difference)
                .collect(),
        })
        .collect()
}

/// `path` relative to the project directory when it is inside it, with `/` separators.
fn project_path(path: &std::path::Path, project_dir: &std::path::Path) -> String {
    path.strip_prefix(project_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// How many tests ran locally, on the warehouse, and could not run locally.
fn run_split(report: &RunOnReport) -> (usize, usize, usize) {
    let refused: usize = report
        .decisions
        .iter()
        .filter(|decision| report.mode == RunOn::Local && !decision.refusals.is_empty())
        .count();
    let local: usize = report
        .decisions
        .iter()
        .filter(|decision| decision.run_on == LOCAL_RUN_ON)
        .count()
        - refused;
    (local, report.decisions.len() - local - refused, refused)
}

/// Write the failures, refusals, failed list, detail and summary lines; returns the exit code and `--json`.
fn complete(
    invocation: &Bound<'_, PyAny>,
    (results, since_json): (&[SqlTestExecutionResult<Bound<'_, PyAny>>], Option<&str>),
    started: Instant,
    report: &RunOnReport,
) -> PyResult<(i32, String)> {
    let py: Python<'_> = invocation.py();
    let project_dir: PathBuf = native_field(invocation, "effective_project_dir")?.extract()?;
    let projector: Option<Bound<'_, NativeProgressProjector>> =
        current_projector(py)?.map(|projector| projector.into_bound(py));
    let mut counts: RunCounts = RunCounts {
        total: results.len(),
        ..RunCounts::default()
    };
    let mut entries: Vec<FailureEntry> = Vec::new();
    let mut failed: Vec<(String, String)> = Vec::new();
    let mut not_run: Vec<&str> = Vec::new();
    for result in results {
        if result.outcome == SqlTestOutcome::Pass {
            counts.pass += 1;
            continue;
        }
        if let Some(code @ (CANNOT_RUN_LOCALLY_CODE | NEEDS_WAREHOUSE_CODE)) =
            result.error_code.as_deref()
        {
            counts.not_run += 1;
            not_run.push(code);
            continue;
        }
        let timed_out: bool = result.outcome == SqlTestOutcome::Timeout;
        if timed_out {
            counts.timeout += 1;
        } else {
            counts.fail += 1;
        }
        let statement: Option<(String, String)> = match &projector {
            Some(projector) => projector.get().failed_statement(&result.test_name)?,
            None => None,
        };
        let facts: TestFailureFacts = TestFailureFacts {
            name: test_result_label(result)?,
            kind: TEST_NOUNS.0.to_owned(),
            run_on: report
                .decisions
                .iter()
                .find(|decision| decision.name == result.test_name)
                .map_or_else(String::new, |decision| decision.run_on.clone()),
            error_code: result.error_code.clone(),
            error_message: result.error_message.clone(),
            error_help: result.error_help.clone(),
            timed_out,
            steps: expectation_steps(result),
            compiled_path: statement
                .as_ref()
                .and_then(|_| report.compiled_paths.get(&result.test_name).cloned()),
            statement,
            source_path: result
                .source_path
                .as_deref()
                .map(|path| project_path(path, &project_dir)),
        };
        let (entry, reason) = test_failure_entry(&facts);
        failed.push((entry.name.clone(), reason));
        entries.push(entry);
    }
    let stream: Bound<'_, PyAny> = invocation.getattr("progress_stream")?;
    let style: CliStyle = CliStyle {
        use_color: invocation.getattr("use_color")?.is_truthy()?,
    };
    let columns: usize = terminal_columns(py)?;
    let mut text: String = String::new();
    let causes: Vec<HeaderCause> = refusal_causes(&report.decisions);
    if !not_run.is_empty() && report.mode == RunOn::Local {
        text.push_str(&render_cant_run_rollup(not_run.len(), &causes, style));
    }
    let moved: Vec<RunOnDecision> = report
        .decisions
        .iter()
        .filter(|decision| {
            report.mode == RunOn::Auto
                && !decision.refusals.is_empty()
                && !report.header_refused.contains(&decision.name)
        })
        .cloned()
        .collect();
    if !moved.is_empty() {
        text.push_str(&render_moved_note(
            moved.len(),
            &refusal_causes(&moved),
            TEST_NOUNS,
            style,
        ));
    }
    if not_run.contains(&NEEDS_WAREHOUSE_CODE) {
        text.push_str(&format!(
            "\n{}\n",
            style.muted(&format!(
                "  Not run ({}): {NEEDS_WAREHOUSE_MESSAGE}",
                not_run
                    .iter()
                    .filter(|code| **code == NEEDS_WAREHOUSE_CODE)
                    .count()
            ))
        ));
    }
    text.push_str(&render_failures(&entries, columns, style));
    text.push_str(&refusal_report(report, (columns, style)));
    text.push_str(&render_failed_list(&failed, report.verbose, style));
    let (local, warehouse, refused) = run_split(report);
    let mut split: Vec<String> = Vec::new();
    if local > 0 {
        split.push(format!("{local} local"));
    }
    if warehouse > 0 {
        split.push(format!("{warehouse} on the warehouse"));
    }
    if refused > 0 {
        split.push(format!("{refused} can't run locally"));
    }
    let mut details: Vec<String> = vec![split.join(", ")];
    let statements: Option<StatementReport> =
        StatementReport::of_invocation(invocation, started, &report.local_statements)?;
    if let Some(line) = statements.as_ref().and_then(StatementReport::line) {
        details.push(line);
    }
    text.push_str(&render_run_completion(
        &counts,
        &details,
        started.elapsed().as_secs_f64(),
        style,
    ));
    emit(&stream, &text)?;
    let payload: String = with_run_on_json(
        &with_since_section(test_execution_json(py, results)?, since_json)?,
        ("checks", &JSON_CHECK_NAME_KEYS),
        &report.decisions,
        (local + refused, warehouse),
    )
    .map_err(value_error)?;
    Ok((
        test_exit_code(counts.pass, results.len()),
        match statements {
            Some(statements) => statements.with_section(&payload)?,
            None => payload,
        },
    ))
}

/// `local`: every refusal as an error; `auto` with `--verbose`: the same blocks as notes; else nothing.
fn refusal_report(report: &RunOnReport, (columns, style): (usize, CliStyle)) -> String {
    match report.mode {
        RunOn::Warehouse => String::new(),
        RunOn::Local => render_refusal_report(
            &report.decisions,
            (true, report.verbose),
            TEST_NOUNS,
            (columns, style),
        ),
        RunOn::Auto if report.verbose => render_refusal_report(
            &report.decisions,
            (false, true),
            TEST_NOUNS,
            (columns, style),
        ),
        RunOn::Auto => String::new(),
    }
}

/// Run one `sqb test` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_test_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    run_test(py, request, None)
}

/// Run one `sqb test` request; a claimed run's tests come from its verified inputs and report to its worker.
pub(crate) fn run_test<'py>(
    py: Python<'py>,
    request: &Bound<'py, PyAny>,
    verified: Option<VerifiedRun<'_, 'py>>,
) -> PyResult<i32> {
    let (worker, discovered) = verified.unzip();
    let inspect_only: bool = native_field(request, "inspect")?.is_truthy()?;
    if native_field(request, "python_only")?.is_truthy()? && worker.is_some() {
        return Err(new_error(
            py,
            ErrorId::PlannerInput,
            "--python cannot run on a server location",
            (
                Some(PYTHON_TESTS_INPUT_CODE),
                Some(
                    "runs on a server location run SQL tests only; run `sqb test --python` locally",
                ),
            ),
        ));
    }
    if native_field(request, "python_only")?.is_truthy()?
        && (inspect_only || !native_field(request, "since")?.is_none())
    {
        return Err(new_error(
            py,
            ErrorId::PlannerInput,
            "--python cannot be combined with --inspect or --since",
            (
                Some(PYTHON_TESTS_INPUT_CODE),
                Some(
                    "--inspect and --since apply to SQL tests only; run `sqb test --python` without them",
                ),
            ),
        ));
    }
    let wants_connection: bool = !inspect_only
        && native_field(request, "run_on")?
            .extract::<Option<String>>()?
            .as_deref()
            != Some(LOCAL_RUN_ON);
    let invocation_class: Bound<'_, PyAny> = record_class(py, RecordId::TestInvocation)?;
    let (invocation, connection_error): (Bound<'_, PyAny>, Option<PyErr>) =
        match resolve_reported_command_invocation(
            request,
            &invocation_class,
            (wants_connection, discovered),
        ) {
            Ok(invocation) => (invocation, None),
            Err(error) if wants_connection => (
                resolve_reported_command_invocation(
                    request,
                    &invocation_class,
                    (false, discovered),
                )
                .map_err(|_| error.clone_ref(py))?,
                Some(error),
            ),
            Err(error) => return Err(error),
        };
    let python: bool = worker.is_none()
        && !inspect_only
        && !native_field(request, "sql_only")?.is_truthy()?
        && native_field(request, "since")?.is_none();
    let modules: Option<Vec<String>> = if python {
        Some(python_test_modules(&invocation)?)
    } else {
        None
    };
    let resolve: bool = match &modules {
        Some(modules) if !modules.is_empty() => {
            names_python_node(request, &native_field(&invocation, "discovered_inputs")?)?
        }
        _ => false,
    };
    let (sql_code, python_tests, payload): (i32, Vec<String>, Option<String>) = match modules {
        Some(modules)
            if native_field(request, "python_only")?.is_truthy()? && !selective(request)? =>
        {
            (
                0,
                selected_python_tests(request, &invocation, (None, modules))?,
                Some(test_execution_json(py, &[])?),
            )
        }
        modules => run_sql_tests(
            request,
            (&invocation, connection_error),
            worker,
            (modules, resolve),
        )?,
    };
    if python_tests.is_empty() {
        write_json(request, payload)?;
        return Ok(sql_code);
    }
    let python_code: i32 = run_python_tests(request, &invocation, &python_tests)?;
    write_json(
        request,
        payload
            .map(|payload| with_python_section(&payload, &python_tests, python_code))
            .transpose()?,
    )?;
    Ok(i32::from(sql_code != 0 || python_code != 0))
}

/// Write a `--json` document to stdout or `--json-output`, when there is one.
fn write_json(request: &Bound<'_, PyAny>, payload: Option<String>) -> PyResult<()> {
    let Some(payload) = payload else {
        return Ok(());
    };
    let py: Python<'_> = request.py();
    write_execution_json_output(
        py,
        PyString::new(py, &payload).into_any(),
        native_field(request, "json_output")?.extract()?,
        native_field(request, "json_output_path")?,
    )?;
    Ok(())
}

/// Compile, inspect or run the selected SQL tests and pick the Python test `modules` to run after.
fn run_sql_tests<'py>(
    request: &Bound<'py, PyAny>,
    (invocation, connection_error): (&Bound<'py, PyAny>, Option<PyErr>),
    worker: Option<Arc<WorkerBuild>>,
    (modules, resolve): (Option<Vec<String>>, bool),
) -> PyResult<(i32, Vec<String>, Option<String>)> {
    let py: Python<'py> = request.py();
    let started: Instant = Instant::now();
    let inspect_only: bool = native_field(request, "inspect")?.is_truthy()?;
    let stream: Bound<'_, PyAny> = invocation.getattr("progress_stream")?;
    let project_dir: Bound<'_, PyAny> = native_field(invocation, "effective_project_dir")?;
    let since: Option<String> = native_field(request, "since")?.extract()?;
    let base: Option<BaseComparison> = match since.as_deref() {
        Some(since) => Some(base_comparison(
            (&stream, &project_dir),
            since,
            &base_variant(
                native_field(request, "selected_target")?.extract()?,
                &native_field(request, "cli_vars")?,
            )?,
        )?),
        None => None,
    };
    let mut pipeline: Bound<'_, PyAny> = compile_tests(request, invocation, resolve)?;
    let python_tests: Vec<String> = match modules {
        Some(modules) => selected_python_tests(request, invocation, (Some(&pipeline), modules))?,
        None => Vec::new(),
    };
    if native_field(request, "python_only")?.is_truthy()? {
        return Ok((0, python_tests, Some(test_execution_json(py, &[])?)));
    }
    let mut reasons: HashMap<String, Vec<String>> = HashMap::new();
    let mut since_json: Option<String> = None;
    if let Some(base) = base {
        let json: String;
        (pipeline, reasons, json) = select_changed(
            (
                &pipeline,
                &stream,
                &native_field(invocation, "discovered_inputs")?,
            ),
            &project_dir.extract::<PathBuf>()?,
            base,
            native_field(request, "explain")?.is_truthy()?,
        )?;
        since_json = Some(json);
    }
    let mode: RunOn = resolve_run_on(
        &request.getattr("run_on")?,
        &native_field(invocation, "discovered_inputs")?,
        &request.getattr("selected_target")?,
        RUN_ON_SETTING,
    )?;
    if mode == RunOn::Warehouse
        && !inspect_only
        && let Some(error) = connection_error.as_ref()
    {
        return Err(error.clone_ref(py));
    }
    let (decisions, compiled_paths): (Vec<RunOnDecision>, HashMap<String, String>) =
        decide(invocation, &pipeline, mode)?;
    if inspect_only {
        let style: CliStyle = CliStyle {
            use_color: invocation.getattr("use_color")?.is_truthy()?,
        };
        let code: i32 = inspect(
            &stream,
            &pipeline.getattr("plan_output")?,
            (&reasons, &decisions),
        )?;
        if mode != RunOn::Warehouse {
            emit(
                &stream,
                &render_refusal_report(
                    &decisions,
                    (mode == RunOn::Local, true),
                    TEST_NOUNS,
                    (terminal_columns(py)?, style),
                ),
            )?;
        }
        return Ok((code, python_tests, None));
    }
    if !python_tests.is_empty()
        && !pipeline
            .getattr("plan_output")?
            .getattr("test_entries")?
            .is_truthy()?
    {
        return Ok((
            0,
            python_tests,
            Some(with_since_section(
                test_execution_json(py, &[])?,
                since_json.as_deref(),
            )?),
        ));
    }
    let concurrency: i64 = resolve_concurrency(
        &request.getattr("concurrency")?,
        &native_field(invocation, "discovered_inputs")?,
        &request.getattr("selected_target")?,
    )?
    .limit;
    let (placement, mut decisions): (TestPlacement<'_>, Vec<RunOnDecision>) = place_tests(
        &pipeline.getattr("plan_output")?.getattr("test_entries")?,
        (mode, decisions),
        connection_error.is_some(),
    )?;
    let style: CliStyle = CliStyle {
        use_color: invocation.getattr("use_color")?.is_truthy()?,
    };
    let explain: bool = native_field(request, "explain")?.is_truthy()?;
    stream.call_method1(
        "write",
        (format!(
            "\n{}\n",
            render_run_header(
                &run_on_header(&decisions, (mode, explain), ("Tests", TEST_NOUNS)),
                terminal_columns(py)?,
                style
            )
        ),),
    )?;
    let header: Bound<'_, PyDict> = PyDict::new(py);
    header.set_item("stream", &stream)?;
    header.set_item("command", EXECUTION_COMMAND)?;
    header.set_item("target", py.None())?;
    header.set_item("concurrency", concurrency)?;
    header.set_item("use_color", invocation.getattr("use_color")?)?;
    let adapter_name: String = native_field(invocation, "adapter_name")?.extract()?;
    header.set_item(
        "engines",
        execution_engines(&decisions, mode, &adapter_name),
    )?;
    write_execution_header(py, Some(header))?;
    let timeout: RunTimeout = resolve_run_timeout(
        (
            &native_field(&pipeline, "project")?,
            &invocation.getattr("discovered_inputs")?,
        ),
        request.getattr("test_timeout")?.extract()?,
        TimeoutKind::Test,
    )?;
    let verbose: bool = native_field(request, "verbose")?.is_truthy()?;
    let (results, local_statements, refused_locally): RanTests<'_> = run_tests(
        invocation,
        &pipeline,
        ((concurrency, &timeout), worker, verbose),
        placement,
    )?;
    let header_refused: HashSet<String> = decisions
        .iter()
        .filter(|decision| !decision.refusals.is_empty())
        .map(|decision| decision.name.clone())
        .collect();
    apply_local_refusals(&mut decisions, refused_locally, mode);
    write_test_runtime_target(
        (
            &native_field(invocation, "effective_project_dir")?
                .call_method1("__truediv__", ("target",))?,
            &invocation.getattr("adapter")?,
        ),
        &pipeline.getattr("plan_output")?,
        &results,
    )?;
    let (code, payload): (i32, String) = complete(
        invocation,
        (&results, since_json.as_deref()),
        started,
        &RunOnReport {
            mode,
            decisions,
            local_statements,
            verbose,
            compiled_paths,
            header_refused,
        },
    )?;
    Ok((code, python_tests, Some(payload)))
}

/// Each selected test's run-on decision, from the local runner, and its compiled SQL path by name.
fn decide(
    invocation: &Bound<'_, PyAny>,
    pipeline: &Bound<'_, PyAny>,
    mode: RunOn,
) -> PyResult<(Vec<RunOnDecision>, HashMap<String, String>)> {
    let py: Python<'_> = invocation.py();
    let entries: Bound<'_, PyAny> = pipeline.getattr("plan_output")?.getattr("test_entries")?;
    let paths: Bound<'_, PyDict> = PyDict::new(py);
    for entry in entries.try_iter()? {
        let entry: Bound<'_, PyAny> = entry?;
        let path: String = planned_sql_test_path(&entry)?.str()?.extract()?;
        paths.set_item(
            entry.getattr("name")?,
            format!("{COMPILED_TESTS_DIR}/{}", path.replace('\\', "/")),
        )?;
    }
    let json: String = call_with(
        &host_entry(py, ExecutorHost::DecideSqlTestRunOn)?,
        &[
            ("project", native_field(pipeline, "project")?),
            ("test_entries", entries),
            ("adapter", invocation.getattr("adapter")?),
            (
                "project_dir",
                native_field(invocation, "effective_project_dir")?,
            ),
            ("compiled_paths", paths.clone().into_any()),
            ("run_on", text(py, mode.as_str())),
            (
                "discovered_inputs",
                native_field(invocation, "discovered_inputs")?,
            ),
        ],
    )?
    .extract()?;
    Ok((
        read_run_on_decisions(&json).map_err(value_error)?,
        paths.extract()?,
    ))
}

/// Split the tests by `mode` and their decisions, and say where each one ran.
fn place_tests<'py>(
    entries: &Bound<'py, PyAny>,
    (mode, decisions): (RunOn, Vec<RunOnDecision>),
    no_connection: bool,
) -> PyResult<(TestPlacement<'py>, Vec<RunOnDecision>)> {
    let mut placement: TestPlacement<'py> = TestPlacement {
        unplaced: Vec::new(),
        local: Vec::new(),
        local_names: Vec::new(),
        warehouse: Vec::new(),
        run_on: HashMap::new(),
        relied_on: HashMap::new(),
        warehouse_optional: mode == RunOn::Auto,
    };
    let mut placed: Vec<RunOnDecision> = Vec::with_capacity(decisions.len());
    for entry in entries.try_iter()? {
        let entry: Bound<'py, PyAny> = entry?;
        let name: String = entry.getattr("name")?.extract()?;
        let mut decision: RunOnDecision = decisions
            .iter()
            .find(|decision| decision.name == name)
            .cloned()
            .unwrap_or_else(|| RunOnDecision {
                name: name.clone(),
                run_on: WAREHOUSE_RUN_ON.to_owned(),
                ..RunOnDecision::default()
            });
        match mode {
            RunOn::Warehouse => {
                decision.run_on = WAREHOUSE_RUN_ON.to_owned();
                placement.warehouse.push(entry);
            }
            _ if decision.local() => {
                placement.local_names.push(name.clone());
                placement.local.push(entry);
            }
            RunOn::Local => {
                decision.run_on = LOCAL_RUN_ON.to_owned();
                placement.unplaced.push((
                    entry,
                    CANNOT_RUN_LOCALLY_CODE,
                    CANNOT_RUN_LOCALLY_MESSAGE,
                ));
            }
            RunOn::Auto if no_connection => {
                placement
                    .unplaced
                    .push((entry, NEEDS_WAREHOUSE_CODE, NEEDS_WAREHOUSE_MESSAGE))
            }
            RunOn::Auto => placement.warehouse.push(entry),
        }
        if let Some(relied) = relied_on_text(&decision.relied_on) {
            placement.relied_on.insert(name.clone(), relied);
        }
        placement.run_on.insert(name, decision.run_on.clone());
        placed.push(decision);
    }
    Ok((placement, placed))
}

/// Add source and safe typed values only for parameterized test rows.
#[pyfunction]
#[pyo3(signature = (**options))]
fn format_parameterized_test_label(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "format_parameterized_test_label",
        options,
        (
            &[
                "name",
                "source_path",
                "parameter_schema",
                "parameter_values",
            ],
            &[],
        ),
    )?;
    let item: Bound<'_, PyAny> = py.import("types")?.getattr("SimpleNamespace")?.call(
        (),
        Some(&{
            let fields: Bound<'_, PyDict> = PyDict::new(py);
            for field in [
                "name",
                "source_path",
                "parameter_schema",
                "parameter_values",
            ] {
                fields.set_item(field, arguments.value(field)?)?;
            }
            fields
        }),
    )?;
    test_label(&item, "name")
}

/// The widest name a test row shows, without its group's prefix.
#[pyfunction]
fn resolve_test_name_width(test_entries: &Bound<'_, PyAny>) -> PyResult<usize> {
    name_width(test_entries)
}

/// Build the child rows of a completed SQL unit test.
#[pyfunction]
fn build_test_expectation_rows<'py>(result: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    expectation_rows(result)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_test_command, module)?)?;
    module.add_function(wrap_pyfunction!(format_parameterized_test_label, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_test_name_width, module)?)?;
    module.add_function(wrap_pyfunction!(build_test_expectation_rows, module)?)?;
    Ok(())
}
