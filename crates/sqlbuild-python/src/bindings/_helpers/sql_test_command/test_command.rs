//! `sqb test`: compile the selected SQL unit tests, inspect or run them, and report; locally, then run the Python tests.

use sqlbuild_python_base::native_host::models::Dialect;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyInt, PyString, PyTuple};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_analysis::definition_changes::models::{ExecutedItem, SelectedItem};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::test_command::main::render_test_completion::render_test_completion;
use sqlbuild_core::test_command::main::render_test_inspection::render_test_inspection;
use sqlbuild_core::test_command::main::render_test_ready::render_test_ready;
use sqlbuild_core::test_command::main::test_exit_code::test_exit_code;
use sqlbuild_core::test_command::main::test_group_name::test_group_name;
use sqlbuild_core::test_command::main::test_name_width::test_name_width;
use sqlbuild_core::test_command::main::test_outcome_status::test_outcome_status;
use sqlbuild_core::test_command::models::{TestFailure, TestInspectionEntry, TestPlanWarning};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::{SqlTestExecutionResult, SqlTestOutcome};
use sqlbuild_host::types::ResultEnum;
use sqlbuild_orchestration::runs::models::NodeOutcome;

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
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_command::main::emit::emit;
use sqlbuild_python_base::build_progress::main::progress::current_native_progress_projector::current_native_progress_projector;
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
use sqlbuild_python_compile::compile_outputs::main::write_test_runtime_target::write_test_runtime_target;
use sqlbuild_python_front::command_runtime::main::resolve_concurrency::resolve_concurrency;
use sqlbuild_python_output::build_progress::main::format_expectation_detail::format_expectation_detail;
use sqlbuild_python_output::build_progress::main::format_expectation_name::format_expectation_name;
use sqlbuild_python_output::build_progress::main::test_label::test_label;
use sqlbuild_python_output::build_progress::main::test_result_label::test_result_label;
use sqlbuild_python_output::build_progress::main::write_execution_header::write_execution_header;
use sqlbuild_python_output::build_progress::models::ConnectionProgressReporter;
use sqlbuild_python_output::build_progress::models::NestedCommandProgressCallbacks;
use sqlbuild_python_output::result_output::main::events::sql_test_identity::sql_test_identity;
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
const EXPECT_LABEL: &str = "expect";
const UNKNOWN_GROUP: &str = "(unknown)";
const EXECUTION_COMMAND: &str = "sqb test";
const UNKNOWN_CASE_CODE: &str = "S008";

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn strings(values: Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

/// The shared name column width of test rows and their expectation rows.
fn name_width(entries: &Bound<'_, PyAny>) -> PyResult<usize> {
    let mut labels: Vec<String> = Vec::new();
    let mut children: Vec<String> = Vec::new();
    for entry in entries.try_iter()? {
        let entry: Bound<'_, PyAny> = entry?;
        labels.push(test_label(&entry, "name")?);
        for step in entry.getattr("chain")?.try_iter()? {
            let step: Bound<'_, PyAny> = step?;
            if step.getattr("expected_cte_sql")?.is_truthy()? {
                children.push(format!(
                    "expected {}",
                    step.getattr("model_name")?.extract::<String>()?
                ));
            }
        }
        for assertion in entry.getattr("assertions")?.try_iter()? {
            children.push(format!(
                "assertion {}",
                assertion?.getattr("name")?.extract::<String>()?
            ));
        }
    }
    Ok(test_name_width(&labels, &children))
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

/// Nested `expect` rows of a completed test, one per step.
fn expectation_rows<'py>(result: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = result.py();
    let mut rows: Vec<Bound<'py, PyAny>> = Vec::new();
    for step in result.getattr("step_results")?.try_iter()? {
        let step: Bound<'py, PyAny> = step?;
        rows.push(new_record_with(
            py,
            RecordId::NestedProgressChildRow,
            &[
                ("label", text(py, EXPECT_LABEL)),
                (
                    "name",
                    text(
                        py,
                        &format_expectation_name(&step.getattr("model_name")?.extract::<String>()?),
                    ),
                ),
                (
                    "status_text",
                    text(py, outcome_status(&step.getattr("outcome")?)?),
                ),
                ("detail", text(py, &format_expectation_detail(&step)?)),
            ],
        )?);
    }
    PyTuple::new(py, rows)
}

/// `N real models` for a model test, else empty.
fn real_model_detail(result: &Bound<'_, PyAny>) -> PyResult<String> {
    let count: Option<usize> = native_field(result, "real_model_count")?.extract()?;
    Ok(match count {
        Some(1) => "  1 real model".to_owned(),
        Some(count) => format!("  {count} real models"),
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

    fn on_test_complete(&self, py: Python<'_>, result: &Bound<'_, PyAny>) -> PyResult<()> {
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
        call_with(
            &self.progress.bind(py).getattr("on_item_complete")?,
            &[
                ("group_name", text(py, &group)),
                ("item_name", text(py, &test_label(result, "test_name")?)),
                ("status_text", text(py, status)),
                ("child_rows", expectation_rows(result)?.into_any()),
                ("detail", text(py, &real_model_detail(result)?)),
                ("error_code", result.getattr("error_code")?),
                ("error_help", result.getattr("error_help")?),
                ("error_message", result.getattr("error_message")?),
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
    })
}

/// Write the resolved plan without executing it; returns the exit code.
fn inspect(
    stream: &Bound<'_, PyAny>,
    plan_output: &Bound<'_, PyAny>,
    reasons: &HashMap<String, Vec<String>>,
) -> PyResult<i32> {
    let entries: Vec<TestInspectionEntry> = native_field(plan_output, "test_entries")?
        .try_iter()?
        .map(|entry| {
            let mut entry: TestInspectionEntry = inspection_entry(&entry?)?;
            entry.selection_reasons = reasons.get(&entry.label).cloned().unwrap_or_default();
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

/// Write the ready header and run the tests with nested progress; returns the results.
fn run_tests<'py>(
    invocation: &Bound<'py, PyAny>,
    pipeline: &Bound<'py, PyAny>,
    (concurrency, timeout, worker): (i64, &RunTimeout, Option<Arc<WorkerBuild>>),
) -> PyResult<Vec<SqlTestExecutionResult<Bound<'py, PyAny>>>> {
    let py: Python<'py> = invocation.py();
    let stream: Bound<'py, PyAny> = invocation.getattr("progress_stream")?;
    let use_color: Bound<'py, PyAny> = invocation.getattr("use_color")?;
    let style: CliStyle = CliStyle {
        use_color: use_color.is_truthy()?,
    };
    let plan_output: Bound<'py, PyAny> = pipeline.getattr("plan_output")?;
    let entries: Bound<'py, PyAny> = plan_output.getattr("test_entries")?;
    let test_count: usize = entries.len()?;
    let mut models: BTreeSet<String> = BTreeSet::new();
    for entry in entries.try_iter()? {
        for step in entry?.getattr("chain")?.try_iter()? {
            models.insert(step?.getattr("model_name")?.extract()?);
        }
    }
    let worker_count: i64 = concurrency.min(i64::try_from(test_count).unwrap_or(i64::MAX));
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
                    ("concurrency", PyInt::new(py, worker_count).into_any()),
                ],
            )?),
        )?,
    )?
    .into_any();
    let projector: Bound<'py, PyAny> = current_native_progress_projector(py)?;
    if !projector.is_none() {
        for entry in entries.try_iter()? {
            call_with(
                &projector.getattr("expect_resource_enrichment")?,
                &[("resource_name", entry?.getattr("name")?)],
            )?;
        }
    }
    stream.call_method1(
        "write",
        (render_test_ready((test_count, models.len()), style),),
    )?;
    stream.call_method0("flush")?;
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
    let relay: Bound<'py, TestCallbackRelay> = Bound::new(
        py,
        TestCallbackRelay {
            worker,
            progress: progress.unbind(),
            event_writer: event_writer.clone().unbind(),
            plan_output: plan_output.clone().unbind(),
            connection: connection.clone().unbind(),
        },
    )?;
    let executed: PyResult<Vec<SqlTestExecutionResult<Bound<'py, PyAny>>>> = (|| {
        let callbacks: Bound<'py, PyAny> = call_with(
            &host_entry(py, ExecutorHost::TestPipelineCallbacks)?,
            &[
                (
                    "on_connection_start",
                    connection.getattr("on_connection_start")?,
                ),
                (
                    "on_connection_complete",
                    relay.getattr("on_connection_complete")?,
                ),
                ("on_connection_error", relay.getattr("on_connection_error")?),
                (
                    "on_progress",
                    preflight.getattr("report_preflight_progress")?,
                ),
                ("on_test_start", relay.getattr("on_test_start")?),
                ("on_test_complete", relay.getattr("on_test_complete")?),
            ],
        )?;
        call_with(
            &host_entry(py, ExecutorHost::RunTestPipeline)?,
            &[
                ("plan", plan_output),
                (
                    "connection_config",
                    invocation.getattr("connection_config")?,
                ),
                ("adapter", invocation.getattr("adapter")?),
                ("max_concurrency", PyInt::new(py, concurrency).into_any()),
                ("callbacks", callbacks),
                ("run_id", pipeline.getattr("project")?.getattr("run_id")?),
                ("timeouts", timeout_record(py, timeout)?),
            ],
        )?
        .try_iter()?
        .map(|result| SqlTestExecutionResult::read(&result?))
        .collect()
    })();
    let closed: PyResult<Bound<'py, PyAny>> = event_writer.call_method0("close");
    let results: Vec<SqlTestExecutionResult<Bound<'py, PyAny>>> = executed?;
    closed?;
    Ok(results)
}

/// The executor's `RunItemTimeouts` for a resolved run limit.
pub(crate) fn timeout_record<'py>(
    py: Python<'py>,
    timeout: &RunTimeout,
) -> PyResult<Bound<'py, PyAny>> {
    host_entry(py, ExecutorHost::RunItemTimeouts)?.call((), Some(&timeout.keywords(py)?))
}

/// Write the summary and failures; returns the exit code and the `--json` document.
fn complete(
    invocation: &Bound<'_, PyAny>,
    (results, since_json): (&[SqlTestExecutionResult<Bound<'_, PyAny>>], Option<&str>),
    started: Instant,
) -> PyResult<(i32, String)> {
    let py: Python<'_> = invocation.py();
    let mut failures: Vec<TestFailure> = Vec::new();
    for result in results {
        if result.outcome == SqlTestOutcome::Pass {
            continue;
        }
        failures.push(TestFailure {
            label: test_result_label(result)?,
            error_message: result.error_message.clone(),
            error_code: result.error_code.clone(),
            error_help: result.error_help.clone(),
        });
    }
    let passed: usize = results.len() - failures.len();
    let stream: Bound<'_, PyAny> = invocation.getattr("progress_stream")?;
    let style: CliStyle = CliStyle {
        use_color: invocation.getattr("use_color")?.is_truthy()?,
    };
    let completion: String = render_test_completion((passed, results.len()), &failures, style);
    emit(&stream, &completion)?;
    let report: Option<StatementReport> = StatementReport::of_invocation(invocation, started)?;
    if let Some(line) = report.as_ref().and_then(StatementReport::line) {
        emit(&stream, &format!("\n{line}\n"))?;
    }
    let payload: String = with_since_section(test_execution_json(py, results)?, since_json)?;
    Ok((
        test_exit_code(passed, results.len()),
        match report {
            Some(report) => report.with_section(&payload)?,
            None => payload,
        },
    ))
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
    let invocation: Bound<'_, PyAny> = resolve_reported_command_invocation(
        request,
        &record_class(py, RecordId::TestInvocation)?,
        (!inspect_only, discovered),
    )?;
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
        modules => run_sql_tests(request, &invocation, worker, (modules, resolve))?,
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
    invocation: &Bound<'py, PyAny>,
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
    if inspect_only {
        return Ok((
            inspect(&stream, &pipeline.getattr("plan_output")?, &reasons)?,
            python_tests,
            None,
        ));
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
    stream.call_method1("write", ("\n",))?;
    let header: Bound<'_, PyDict> = PyDict::new(py);
    header.set_item("stream", &stream)?;
    header.set_item("command", EXECUTION_COMMAND)?;
    header.set_item("target", py.None())?;
    header.set_item("concurrency", concurrency)?;
    header.set_item("use_color", invocation.getattr("use_color")?)?;
    write_execution_header(py, Some(header))?;
    let timeout: RunTimeout = resolve_run_timeout(
        (
            &native_field(&pipeline, "project")?,
            &invocation.getattr("discovered_inputs")?,
        ),
        request.getattr("test_timeout")?.extract()?,
        TimeoutKind::Test,
    )?;
    let results: Vec<SqlTestExecutionResult<Bound<'_, PyAny>>> =
        run_tests(invocation, &pipeline, (concurrency, &timeout, worker))?;
    write_test_runtime_target(
        (
            &native_field(invocation, "effective_project_dir")?
                .call_method1("__truediv__", ("target",))?,
            &invocation.getattr("adapter")?,
        ),
        &pipeline.getattr("plan_output")?,
        &results,
    )?;
    let (code, payload): (i32, String) =
        complete(invocation, (&results, since_json.as_deref()), started)?;
    Ok((code, python_tests, Some(payload)))
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

/// Resolve one shared width for test rows and nested expectation rows.
#[pyfunction]
fn resolve_test_name_width(test_entries: &Bound<'_, PyAny>) -> PyResult<usize> {
    name_width(test_entries)
}

/// Build nested expectation rows for a completed SQL unit test.
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
