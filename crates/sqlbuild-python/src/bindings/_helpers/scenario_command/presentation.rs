//! Scenario progress: section headers, live status, per-scenario rows and connection timing.

use std::collections::HashSet;
use std::path::PathBuf;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFloat, PyTuple};
use pyo3::{pyclass, pymethods};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::nested_progress::models::NestedChildRow;
use sqlbuild_core::scenario_command::main::render_capture_result::render_capture_result;
use sqlbuild_core::scenario_command::main::render_snapshot_sync_result::render_snapshot_sync_result;
use sqlbuild_core::scenario_command::main::scenario_activity_message::scenario_activity_message;
use sqlbuild_core::scenario_command::main::scenario_steps::scenario_steps;
use sqlbuild_core::scenario_command::models::{CodedText, ScenarioNamespace, ScenarioResultRow};
use sqlbuild_core::test_command::main::expectation_child_rows::expectation_child_rows;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::ScenarioRunResult;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_output::build_progress::main::local_attempt::local_attempt;
use sqlbuild_python_output::build_progress::models::LocalAttemptStep;

use crate::bindings::_helpers::scenario_command::conversions::{
    capture_row, scenario_result_row, write_local_runtime_target, write_warehouse_runtime_target,
};
use crate::bindings::_helpers::scenario_command::setup::{
    boolean, flush, stream_isatty, text, write,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::build_progress::main::transient_status::status_reporter::status_reporter;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::types::ReadResult;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_output::build_progress::models::ConnectionProgressReporter;

/// How a finished scenario is reported.
#[derive(Clone, Debug)]
pub(crate) enum CompletionKind {
    /// A test run: write its runtime SQL under `target_dir` and its result row.
    Run { target_dir: PathBuf },
    /// A snapshot capture: its capture row.
    Capture,
    /// A local snapshot sync or refresh: its sync row.
    Sync { refresh: bool },
}

/// Starts the live status per scenario and reports each finished scenario.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ScenarioProgressRelay {
    stream: Py<PyAny>,
    status: Py<PyAny>,
    status_is_tty: bool,
    activity: String,
    use_color: bool,
    project_dir: Py<PyAny>,
    adapter: Py<PyAny>,
    kind: CompletionKind,
    /// The run's row reporter, which a test run reports each scenario to.
    progress: Option<Py<PyAny>>,
    /// Scenarios that can't run locally under `--run-on local`, by name.
    refused: HashSet<String>,
    local_only: bool,
}

#[pymethods]
impl ScenarioProgressRelay {
    fn on_scenario_start(&self, py: Python<'_>, scenario: Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(progress) = &self.progress {
            call_with(
                &progress.bind(py).getattr("on_item_start")?,
                &[
                    ("group_name", text(py, "")),
                    ("item_name", native_field(&scenario, "name")?),
                ],
            )?;
            return Ok(());
        }
        if self.status_is_tty {
            self.status
                .bind(py)
                .call_method1("start", (self.activity.as_str(),))?;
        }
        Ok(())
    }

    fn on_scenario_complete(
        &self,
        py: Python<'_>,
        _scenario: Bound<'_, PyAny>,
        scenario_plan: Bound<'_, PyAny>,
        result: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if self.status_is_tty && self.progress.is_none() {
            self.status.bind(py).call_method0("close")?;
        }
        let style: CliStyle = CliStyle {
            use_color: self.use_color,
        };
        if local_attempt(LocalAttemptStep::SettleReplay(&result))? {
            return Ok(());
        }
        let plan: Option<&Bound<'_, PyAny>> = (!scenario_plan.is_none()).then_some(&scenario_plan);
        let project_dir: &Bound<'_, PyAny> = self.project_dir.bind(py);
        let rendered: String = match &self.kind {
            CompletionKind::Run { target_dir } => {
                if let Some(plan) = plan {
                    let adapter: &Bound<'_, PyAny> = self.adapter.bind(py);
                    if native_field(&result, "local_status")?.is_none() {
                        write_warehouse_runtime_target(target_dir, (adapter, plan, &result))?;
                    } else {
                        write_local_runtime_target(target_dir, (adapter, plan, &result))?;
                    }
                }
                let row: ScenarioResultRow =
                    scenario_result_row(py, &ScenarioRunResult::read(&result)?)?;
                let refused: bool = self.local_only
                    && (self.refused.contains(&row.name)
                        || !native_field(&result, "local_refusal")?.is_none());
                return match &self.progress {
                    Some(progress) => report_row(progress.bind(py), &row, refused),
                    None => Ok(()),
                };
            }
            CompletionKind::Capture => {
                render_capture_result(&capture_row(&result, (project_dir, plan))?, style)
            }
            CompletionKind::Sync { refresh } => render_snapshot_sync_result(
                &capture_row(&result, (project_dir, plan))?,
                *refresh,
                style,
            ),
        };
        let stream: &Bound<'_, PyAny> = self.stream.bind(py);
        write(stream, &rendered)?;
        flush(stream)
    }
}

/// The errors of a scenario whose expected rows differ (`X506`) or whose assertion returned rows (`X508`); its child rows already say which.
const EXPECTATIONS_FAILED_CODES: [&str; 2] = ["X506", "X508"];

/// One scenario's row on the run's reporter: its status, where it ran, its error and expectations.
fn report_row(progress: &Bound<'_, PyAny>, row: &ScenarioResultRow, not_run: bool) -> PyResult<()> {
    let py: Python<'_> = progress.py();
    let children: Vec<NestedChildRow> = expectation_child_rows(&scenario_steps(row));
    let error: Option<&CodedText> = row
        .error
        .as_ref()
        .filter(|error| !error.message.is_empty())
        .filter(|error| {
            children.is_empty()
                || !error
                    .code
                    .as_deref()
                    .is_some_and(|code| EXPECTATIONS_FAILED_CODES.contains(&code))
        });
    let optional = |value: Option<&str>| match value {
        Some(value) => text(py, value),
        None => py.None().into_bound(py),
    };
    call_with(
        &progress.getattr("on_item_complete")?,
        &[
            ("group_name", text(py, "")),
            ("item_name", text(py, &row.name)),
            ("status_text", text(py, &row.status)),
            (
                "run_on",
                text(py, if row.local { "local" } else { "warehouse" }),
            ),
            ("not_run", boolean(py, not_run)),
            (
                "seconds",
                match row.duration_ms {
                    Some(milliseconds) => PyFloat::new(py, milliseconds / 1000.0).into_any(),
                    None => py.None().into_bound(py),
                },
            ),
            (
                "error_code",
                optional(error.and_then(|error| error.code.as_deref())),
            ),
            (
                "error_message",
                optional(error.map(|error| error.message.as_str())),
            ),
            (
                "error_help",
                optional(error.and_then(|error| error.help.as_deref())),
            ),
            ("child_rows", child_records(py, &children)?.into_any()),
            ("canonical_resource_name", text(py, &row.name)),
        ],
    )?;
    Ok(())
}

/// The reporter's records of the child rows under one item.
pub(crate) fn child_records<'py>(
    py: Python<'py>,
    rows: &[NestedChildRow],
) -> PyResult<Bound<'py, PyTuple>> {
    let mut records: Vec<Bound<'py, PyAny>> = Vec::with_capacity(rows.len());
    for row in rows {
        records.push(new_record_with(
            py,
            RecordId::NestedProgressChildRow,
            &[
                ("label", text(py, &row.label)),
                ("name", text(py, &row.name)),
                ("status_text", text(py, &row.status_text)),
                ("detail", text(py, &row.detail)),
            ],
        )?);
    }
    PyTuple::new(py, records)
}

/// Forward connection timing to a reporter that takes keywords.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ScenarioConnectionRelay {
    reporter: Py<PyAny>,
}

#[pymethods]
impl ScenarioConnectionRelay {
    fn on_connection_complete(
        &self,
        py: Python<'_>,
        connection_count: i64,
        elapsed_seconds: f64,
    ) -> PyResult<()> {
        self.forward(
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
        self.forward(
            py,
            "on_connection_error",
            (connection_count, elapsed_seconds),
        )
    }
}

impl ScenarioConnectionRelay {
    fn forward(
        &self,
        py: Python<'_>,
        method: &str,
        (connection_count, elapsed_seconds): (i64, f64),
    ) -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("connection_count", connection_count)?;
        options.set_item("elapsed_seconds", elapsed_seconds)?;
        self.reporter
            .bind(py)
            .call_method(method, (), Some(&options))?;
        Ok(())
    }
}

/// `ConnectionHooks` reporting connection progress for `adapter_name` on `stream`.
pub(crate) fn connection_hooks<'py>(
    stream: &Bound<'py, PyAny>,
    (adapter_name, style): (&str, CliStyle),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = stream.py();
    let reporter: Bound<'py, PyAny> = Bound::new(
        py,
        ConnectionProgressReporter::new(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter_name", text(py, adapter_name)),
                    ("blank_line_after_complete", boolean(py, true)),
                    ("stream", stream.clone()),
                    ("use_color", boolean(py, style.use_color)),
                ],
            )?),
        )?,
    )?
    .into_any();
    let relay: Bound<'py, ScenarioConnectionRelay> = Bound::new(
        py,
        ScenarioConnectionRelay {
            reporter: reporter.clone().unbind(),
        },
    )?;
    call_with(
        &host_entry(py, ExecutorHost::ConnectionHooks)?,
        &[
            (
                "on_connection_start",
                native_field(&reporter, "on_connection_start")?,
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
    )
}

/// The live status of one scenario section and whether it animates on a terminal.
pub(crate) struct SectionStatus<'py> {
    pub(crate) status: Bound<'py, PyAny>,
    pub(crate) status_is_tty: bool,
    pub(crate) activity: String,
}

/// Write the section header and preamble, then the activity line when not on a terminal.
pub(crate) fn begin_section<'py>(
    stream: &Bound<'py, PyAny>,
    (header, preamble, activity): (&str, &str, String),
    style: CliStyle,
) -> PyResult<SectionStatus<'py>> {
    write(
        stream,
        &format!("\n{}\n\n{preamble}", style.success_strong(header)),
    )?;
    flush(stream)?;
    let status: Bound<'py, PyAny> = status_reporter(stream, style.use_color)?;
    let status_is_tty: bool = stream_isatty(stream)?;
    if !status_is_tty {
        write(stream, &format!("{activity}\n\n"))?;
        flush(stream)?;
    }
    Ok(SectionStatus {
        status,
        status_is_tty,
        activity,
    })
}

/// The section status of a test run, whose rows and footer the run's row reporter draws instead.
pub(crate) fn quiet_section<'py>(
    stream: &Bound<'py, PyAny>,
    style: CliStyle,
) -> PyResult<SectionStatus<'py>> {
    Ok(SectionStatus {
        status: status_reporter(stream, style.use_color)?,
        status_is_tty: false,
        activity: String::new(),
    })
}

/// The project directory, adapter, completion kind and style a progress relay reports with.
pub(crate) struct RelayContext<'a, 'py> {
    pub(crate) project_dir: &'a Bound<'py, PyAny>,
    pub(crate) adapter: &'a Bound<'py, PyAny>,
    pub(crate) kind: CompletionKind,
    pub(crate) style: CliStyle,
    /// A test run's row reporter, the scenarios that can't run locally, and whether that is an error.
    pub(crate) rows: Option<(&'a Bound<'py, PyAny>, HashSet<String>, bool)>,
}

/// The relay reporting one section's scenarios.
pub(crate) fn progress_relay<'py>(
    stream: &Bound<'py, PyAny>,
    section: &SectionStatus<'py>,
    RelayContext {
        project_dir,
        adapter,
        kind,
        style,
        rows,
    }: RelayContext<'_, 'py>,
) -> PyResult<Bound<'py, ScenarioProgressRelay>> {
    let (progress, refused, local_only) = match rows {
        Some((progress, refused, local_only)) => {
            (Some(progress.clone().unbind()), refused, local_only)
        }
        None => (None, HashSet::new(), false),
    };
    Bound::new(
        stream.py(),
        ScenarioProgressRelay {
            stream: stream.clone().unbind(),
            status: section.status.clone().unbind(),
            status_is_tty: section.status_is_tty,
            activity: section.activity.clone(),
            use_color: style.use_color,
            project_dir: project_dir.clone().unbind(),
            adapter: adapter.clone().unbind(),
            kind,
            progress,
            refused,
            local_only,
        },
    )
}

/// `<activity> (namespace: ...; source: ...)`.
pub(crate) fn activity(activity: &str, namespace: &ScenarioNamespace) -> String {
    scenario_activity_message(activity, namespace)
}

/// The explicit terminal state of the namespaced run.
pub(crate) fn write_completion(
    stream: &Bound<'_, PyAny>,
    (namespace, succeeded): (&ScenarioNamespace, bool),
) -> PyResult<()> {
    let state: &str = if succeeded {
        "Scenario execution complete"
    } else {
        "Scenario execution failed"
    };
    write(stream, &format!("{}\n", activity(state, namespace)))?;
    flush(stream)
}
