//! `sqb audit`: compile the selected audits, run them with nested progress, and report.

use std::collections::BTreeSet;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyInt, PyString};
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};
use sqlbuild_analysis::assembly::target_checks::constants::PRESERVE_TARGET_VALUE;
use sqlbuild_core::audit_command::main::audit_exit_code::audit_exit_code;
use sqlbuild_core::audit_command::main::render_audit_header::render_audit_header;
use sqlbuild_core::audit_command::main::render_audit_summary::render_audit_summary;
use sqlbuild_core::audit_command::models::AuditCounts;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::{AuditExecutionResult, AuditOutcome};

use crate::bindings::_helpers::boundary::python_calls::{call_with, keywords, within};
use crate::bindings::_helpers::build_progress::audit_progress::AuditProgressReporter;
use crate::bindings::_helpers::build_progress::progress::current_native_progress_projector;
use crate::bindings::_helpers::build_progress::progress_reporters::ConnectionProgressReporter;
use crate::bindings::_helpers::build_progress::progress_text::write_execution_header;
use crate::bindings::_helpers::command_runtime::concurrency::resolve_concurrency;
use crate::bindings::_helpers::command_runtime::invocation::resolve_reported_command_invocation;
use crate::bindings::_helpers::compile_driver::pipeline_entries::build_static_pipeline_result;
use crate::bindings::_helpers::compile_driver::static_context::compile_static_command_context;
use crate::bindings::_helpers::cost_telemetry::context::CostContext;
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::results::ReadResult;
use crate::bindings::_helpers::planner_entries::command_plans::build_audit_command_plan;
use crate::bindings::_helpers::records::construct::{new_record_with, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::result_output::documents::{
    audit_execution_json, audit_projection, write_execution_json_output,
};
use crate::bindings::_helpers::result_output::event_writer::ExecutionEventWriter;

const EXECUTION_COMMAND: &str = "sqb audit";

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

/// A target database or schema as a storage location: `preserve` names no location, so audit history is stored beside the models, as a build stores it.
fn literal_location<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if !value.is_none() && value.extract::<String>()? == PRESERVE_TARGET_VALUE {
        return Ok(py.None().into_bound(py));
    }
    Ok(value.clone())
}

/// Forward audit results to the event writer and connection timing to its reporter.
#[pyclass(module = "sqlbuild._native", frozen)]
struct AuditCallbackRelay {
    event_writer: Py<PyAny>,
    plan: Py<PyAny>,
    connection: Py<PyAny>,
}

#[pymethods]
impl AuditCallbackRelay {
    fn on_result(&self, py: Python<'_>, result: Bound<'_, PyAny>) -> PyResult<()> {
        call_with(
            &self.event_writer.bind(py).getattr("write_build_result")?,
            &[
                ("result", result),
                ("plan", self.plan.bind(py).clone()),
                ("command", text(py, "audit")),
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

impl AuditCallbackRelay {
    fn forward(
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

/// Compile the audit plan for the selected scope.
fn compile_audits<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let discovered: Bound<'py, PyAny> = native_field(invocation, "discovered_inputs")?;
    let adapter: Bound<'py, PyAny> = native_field(invocation, "adapter")?;
    let resolver: Bound<'py, PyAny> = resolve_external_sql_reference_resolver(
        native_field(invocation, "effective_project_dir")?,
        discovered.clone(),
    )?;
    let options: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CompilePipelineOptions,
        &[
            ("selected_target", native_field(request, "selected_target")?),
            (
                "no_sql_validation",
                native_field(request, "no_sql_validation")?,
            ),
            ("defer_to", native_field(request, "defer_to")?),
            ("select", native_field(request, "select")?),
            ("exclude", native_field(request, "exclude")?),
            (
                "connection_config",
                native_field(invocation, "connection_config")?,
            ),
            ("cli_vars", native_field(request, "cli_vars")?),
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
                native_field(invocation, "planning_progress")?.getattr("on_progress")?,
            ),
        ],
    )?))?;
    let plan_output: Bound<'py, PyAny> = build_audit_command_plan(
        &native_field(&context, "project")?,
        &adapter,
        &native_field(&context, "scope")?,
        &native_field(&context, "relations")?,
    )?;
    build_static_pipeline_result(
        py,
        Some(keywords(
            py,
            &[("context", context), ("plan_output", plan_output)],
        )?),
    )
}

/// The audits run with their progress and connection reporters.
struct AuditRun<'py> {
    progress: Bound<'py, PyAny>,
    connection: Bound<'py, PyAny>,
    concurrency: i64,
    worker_count: i64,
}

/// Create the nested progress and connection reporters before the execution header.
fn prepare_run<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Bound<'py, PyAny>,
    pipeline: &Bound<'py, PyAny>,
) -> PyResult<AuditRun<'py>> {
    let py: Python<'py> = request.py();
    let entries: Bound<'py, PyAny> =
        native_field(pipeline, "plan_output")?.getattr("audit_entries")?;
    let concurrency: i64 = resolve_concurrency(
        &native_field(request, "concurrency")?,
        &native_field(invocation, "discovered_inputs")?,
        &native_field(request, "selected_target")?,
    )?
    .limit;
    let audit_count: usize = entries.len()?;
    let worker_count: i64 = concurrency.min(i64::try_from(audit_count).unwrap_or(i64::MAX));
    let stream: Bound<'py, PyAny> = native_field(invocation, "progress_stream")?;
    let use_color: Bound<'py, PyAny> = native_field(invocation, "use_color")?;
    let progress: Bound<'py, PyAny> = AuditProgressReporter::new(
        py,
        Some(keywords(
            py,
            &[
                ("entries", entries.clone()),
                ("worker_limit", PyInt::new(py, worker_count).into_any()),
                ("stream", stream.clone()),
                ("use_color", use_color.clone()),
            ],
        )?),
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let projector: Bound<'py, PyAny> = current_native_progress_projector(py)?;
    if !projector.is_none() {
        for entry in entries.try_iter()? {
            call_with(
                &native_field(&projector, "expect_resource_enrichment")?,
                &[("resource_name", native_field(&entry?, "name")?)],
            )?;
        }
    }
    let connection: Bound<'py, PyAny> = ConnectionProgressReporter::new(
        py,
        Some(keywords(
            py,
            &[
                ("adapter_name", native_field(invocation, "adapter_name")?),
                ("stream", stream),
                ("use_color", use_color),
            ],
        )?),
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    Ok(AuditRun {
        progress,
        connection,
        concurrency,
        worker_count,
    })
}

/// Write the execution header, then the selected audits header.
fn write_headers(
    invocation: &Bound<'_, PyAny>,
    pipeline: &Bound<'_, PyAny>,
    concurrency: i64,
) -> PyResult<()> {
    let py: Python<'_> = invocation.py();
    let stream: Bound<'_, PyAny> = native_field(invocation, "progress_stream")?;
    let use_color: Bound<'_, PyAny> = native_field(invocation, "use_color")?;
    stream.call_method1("write", ("\n",))?;
    let header: Bound<'_, PyDict> = PyDict::new(py);
    header.set_item("stream", &stream)?;
    header.set_item("command", EXECUTION_COMMAND)?;
    header.set_item("target", py.None())?;
    header.set_item("concurrency", concurrency)?;
    header.set_item("use_color", &use_color)?;
    write_execution_header(py, Some(header.clone()))
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let entries: Bound<'_, PyAny> =
        native_field(pipeline, "plan_output")?.getattr("audit_entries")?;
    let mut models: BTreeSet<String> = BTreeSet::new();
    for entry in entries.try_iter()? {
        let target: Bound<'_, PyAny> = native_field(&entry?, "attached_target_name")?;
        if target.is_truthy()? {
            models.insert(target.extract()?);
        }
    }
    let style: CliStyle = CliStyle {
        use_color: use_color.is_truthy()?,
    };
    stream.call_method1(
        "write",
        (render_audit_header((entries.len()?, models.len()), style),),
    )?;
    stream.call_method0("flush")?;
    Ok(())
}

/// Run the audits inside the run's cost scope; returns their results.
fn execute<'py>(
    invocation: &Bound<'py, PyAny>,
    pipeline: &Bound<'py, PyAny>,
    run: &AuditRun<'py>,
) -> PyResult<Vec<AuditExecutionResult<Bound<'py, PyAny>>>> {
    let py: Python<'py> = invocation.py();
    let plan: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    let project: Bound<'py, PyAny> = native_field(pipeline, "project")?;
    let event_writer: Bound<'py, PyAny> = ExecutionEventWriter::new(py, None)
        .and_then(|value| pyo3::Bound::new(py, value))
        .map(pyo3::Bound::into_any)?;
    let relay: Bound<'py, AuditCallbackRelay> = Bound::new(
        py,
        AuditCallbackRelay {
            event_writer: event_writer.clone().unbind(),
            plan: plan.clone().unbind(),
            connection: run.connection.clone().unbind(),
        },
    )?;
    let executed: PyResult<Vec<AuditExecutionResult<Bound<'py, PyAny>>>> = (|| {
        run.progress
            .call_method1("set_result_callback", (native_field(&relay, "on_result")?,))?;
        let projector: Bound<'py, PyAny> = current_native_progress_projector(py)?;
        if !projector.is_none() {
            let ordinals: Bound<'py, PyDict> = PyDict::new(py);
            let mut total: usize = 0;
            for entry in native_field(&plan, "audit_entries")?.try_iter()? {
                total += 1;
                ordinals.set_item(native_field(&entry?, "name")?, total)?;
            }
            call_with(
                &native_field(&projector, "configure_resources")?,
                &[
                    ("ordinals", ordinals.into_any()),
                    ("total", PyInt::new(py, total).into_any()),
                ],
            )?;
        }
        let run_id: Bound<'py, PyAny> = native_field(&project, "run_id")?;
        let ledger_path: Bound<'py, PyAny> = native_field(invocation, "effective_project_dir")?
            .call_method1("__truediv__", ("target",))?
            .call_method1("__truediv__", ("executions",))?
            .call_method1("__truediv__", (&run_id,))?
            .call_method1("__truediv__", ("statements.jsonl",))?;
        let scope: Bound<'py, PyAny> = CostContext::scope(
            py,
            Some(keywords(
                py,
                &[
                    ("run_id", run_id.clone()),
                    ("resource_type", text(py, "run")),
                    ("resource_name", text(py, "audit")),
                    ("ledger_path", ledger_path),
                    ("phase", text(py, "audit")),
                ],
            )?),
        )
        .and_then(|scope| Bound::new(py, scope))?
        .into_any();
        within(&scope, |_| {
            let callbacks: Bound<'py, PyAny> = call_with(
                &host::entry(py, ExecutorHost::AuditPipelineCallbacks)?,
                &[
                    (
                        "on_connection_start",
                        native_field(&run.connection, "on_connection_start")?,
                    ),
                    (
                        "on_connection_complete",
                        native_field(&relay, "on_connection_complete")?,
                    ),
                    (
                        "on_connection_error",
                        native_field(&relay, "on_connection_error")?,
                    ),
                    (
                        "on_audit_start",
                        native_field(&run.progress, "on_item_start")?,
                    ),
                    (
                        "on_audit_physical_complete",
                        native_field(&run.progress, "on_item_physical_complete")?,
                    ),
                    (
                        "on_audit_complete",
                        native_field(&run.progress, "on_item_complete")?,
                    ),
                    (
                        "on_audit_error",
                        native_field(&run.progress, "on_item_error")?,
                    ),
                ],
            )?;
            call_with(
                &host::entry(py, ExecutorHost::RunAuditPipelineWithCallbacks)?,
                &[
                    ("plan", plan.clone()),
                    (
                        "connection_config",
                        native_field(invocation, "connection_config")?,
                    ),
                    ("adapter", native_field(invocation, "adapter")?),
                    (
                        "max_concurrency",
                        PyInt::new(py, run.concurrency).into_any(),
                    ),
                    ("callbacks", callbacks),
                    ("run_id", run_id.clone()),
                    (
                        "storage_database",
                        literal_location(&native_field(&project, "effective_target_database")?)?,
                    ),
                    (
                        "storage_schema",
                        literal_location(&native_field(&project, "effective_target_schema")?)?,
                    ),
                ],
            )?
            .try_iter()?
            .map(|result| AuditExecutionResult::read(&result?))
            .collect()
        })
    })();
    let progress_closed: PyResult<Bound<'py, PyAny>> = run.progress.call_method0("close");
    let writer_closed: PyResult<Bound<'py, PyAny>> = event_writer.call_method0("close");
    let results = executed?;
    progress_closed?;
    writer_closed?;
    Ok(results)
}

/// The audit JSON document, with the result projection counts when audits were projected.
fn audit_payload<'py>(
    py: Python<'py>,
    results: &[AuditExecutionResult<Bound<'py, PyAny>>],
    (concurrency, worker_count): (i64, i64),
) -> PyResult<String> {
    let projection: Bound<'py, PyAny> =
        host::entry(py, ExecutorHost::CurrentAuditResultProjection)?.call0()?;
    let trailing = if projection.is_none() {
        Vec::new()
    } else {
        audit_projection(
            py,
            (
                native_field(&projection, "attempted_count")?.extract()?,
                native_field(&projection, "written_count")?.extract()?,
                native_field(&projection, "failed_count")?.extract()?,
            ),
        )?
    };
    audit_execution_json(
        py,
        results,
        (
            Some(PyInt::new(py, concurrency).into_any()),
            Some(PyInt::new(py, worker_count).into_any()),
        ),
        trailing,
    )
}

/// The warning line for audit results that could not be stored in the warehouse history table; audit outcomes are unaffected.
fn projection_warning(py: Python<'_>, style: CliStyle) -> PyResult<Option<String>> {
    let projection: Bound<'_, PyAny> =
        host::entry(py, ExecutorHost::CurrentAuditResultProjection)?.call0()?;
    if projection.is_none() || !native_field(&projection, "degraded")?.is_truthy()? {
        return Ok(None);
    }
    let failed: i64 = native_field(&projection, "failed_count")?.extract()?;
    let reason: Option<String> = native_field(&projection, "failure_reason")?.extract()?;
    let reason: String = reason.map_or_else(String::new, |reason| format!(": {reason}"));
    Ok(Some(format!(
        "{} {failed} audit result(s) were not written to the audit history table{reason}\n",
        style.warning("warning:")
    )))
}

/// Write the summary footer and JSON output; returns the exit code.
fn complete(
    request: &Bound<'_, PyAny>,
    invocation: &Bound<'_, PyAny>,
    results: &[AuditExecutionResult<Bound<'_, PyAny>>],
    run: &AuditRun<'_>,
) -> PyResult<i32> {
    let py: Python<'_> = request.py();
    let mut counts: AuditCounts = AuditCounts {
        total: results.len(),
        ..AuditCounts::default()
    };
    for result in results {
        match result.outcome {
            AuditOutcome::Pass => counts.passed += 1,
            AuditOutcome::Warn => counts.warned += 1,
            AuditOutcome::Error => counts.failed += 1,
            AuditOutcome::Insufficient => counts.insufficient += 1,
            AuditOutcome::Skipped => {}
        }
    }
    let stream: Bound<'_, PyAny> = native_field(invocation, "progress_stream")?;
    let style: CliStyle = CliStyle {
        use_color: native_field(invocation, "use_color")?.is_truthy()?,
    };
    let warning: String = projection_warning(py, style)?.unwrap_or_default();
    stream.call_method1(
        "write",
        (format!("{warning}{}", render_audit_summary(counts, style)),),
    )?;
    stream.call_method0("flush")?;
    let payload = audit_payload(py, results, (run.concurrency, run.worker_count))?;
    write_execution_json_output(
        py,
        PyString::new(py, &payload).into_any(),
        native_field(request, "json_output")?.extract::<bool>()?,
        native_field(request, "json_output_path")?,
    )?;
    Ok(audit_exit_code(counts.failed))
}

/// Run one `sqb audit` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_audit_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let invocation: Bound<'_, PyAny> = resolve_reported_command_invocation(
        request,
        &record_class(py, RecordId::AuditInvocation)?,
        true,
    )?;
    let pipeline: Bound<'_, PyAny> = compile_audits(request, &invocation)?;
    let run: AuditRun<'_> = prepare_run(request, &invocation, &pipeline)?;
    write_headers(&invocation, &pipeline, run.concurrency)?;
    let results = execute(&invocation, &pipeline, &run)?;
    complete(request, &invocation, &results, &run)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_audit_command, module)?)?;
    Ok(())
}
