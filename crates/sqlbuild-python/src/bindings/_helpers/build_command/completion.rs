//! Cost finalization, runtime targets, the footer, execution JSON and phase timings.

use std::time::Instant;

use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_core::build_command::main::build_exit_code::build_exit_code;
use sqlbuild_core::build_command::main::python_node_failed::python_node_failed;
use sqlbuild_core::build_command::main::render_phase_timings::render_phase_timings;
use sqlbuild_core::build_command::models::PhaseTimings;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::BuildStatus;

use crate::bindings::_helpers::boundary::python_calls::{call_with, keywords};
use crate::bindings::_helpers::build_command::execution::{
    Outcome, Preparation, write_run_context,
};
use crate::bindings::_helpers::build_command::invocation::BuildContext;
use crate::bindings::_helpers::build_command::support::{
    boolean, emit, joined, none, text, utc_now,
};
use crate::bindings::_helpers::build_progress::footer::format_build_footer_with;
use crate::bindings::_helpers::build_progress::result_text::build_result_text;
use crate::bindings::_helpers::check::check_results::check_result_text;
use crate::bindings::_helpers::compile_driver::pipeline_entries::plan_has_executable_work;
use crate::bindings::_helpers::compile_outputs::runtime_target::{
    write_python_check_runtime_target, write_runtime_target,
};
use crate::bindings::_helpers::cost_report::degradation::{
    finalize_build_cost as finalize_cost, render_build_cost,
};
use crate::bindings::_helpers::cost_report::functions::cost_output_payload;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::result_output::documents::format_build_execution_json;
use crate::bindings::_helpers::result_output::documents::format_build_execution_json_with;
use crate::bindings::_helpers::result_output::documents::write_execution_json_output;
use sqlbuild_core::build_progress::models::PythonNodeText;
use sqlbuild_core::check_command::models::{CheckResultText, CheckStatus};

const SUCCESS_STATUS: &str = "success";
const CHECK_KIND: &str = "check";
const CHECK_WARNED: &str = "check warned";
const PENDING_MESSAGE: &str = "Build cost collection has not completed.";
const INTERRUPTED_MESSAGE: &str = "Build was interrupted before cost collection completed.";

/// How one cost finalization differs from a completed collection.
pub(crate) struct CostRequest<'py> {
    pub(crate) status: &'static str,
    pub(crate) started_at: Bound<'py, PyAny>,
    pub(crate) completed_at: Bound<'py, PyAny>,
    pub(crate) overrides: Vec<(&'static str, Bound<'py, PyAny>)>,
}

/// `finalize_build_cost(BuildCostFinalization(...))`.
pub(crate) fn finalize_build_cost<'py>(
    context: &BuildContext<'py>,
    pipeline: &Bound<'py, PyAny>,
    cost: CostRequest<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let project: Bound<'py, PyAny> = native_field(pipeline, "project")?;
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("project_dir", context.get("effective_project_dir")?),
        ("adapter_name", context.get("adapter_name")?),
        ("adapter", context.get("adapter")?),
        ("connection_config", context.get("connection_config")?),
        (
            "target_name",
            native_field(&project, "effective_target_name")?,
        ),
        (
            "target_database",
            native_field(&project, "effective_target_database")?,
        ),
        ("run_id", native_field(&project, "run_id")?),
        ("build_status", text(py, cost.status)),
        ("started_at", cost.started_at),
        ("completed_at", cost.completed_at),
        (
            "config",
            context
                .get("discovered_inputs")?
                .getattr("project_config")?
                .getattr("cost")?,
        ),
        ("output_stream", context.stream.clone()),
        ("use_color", boolean(py, context.use_color)),
    ];
    fields.extend(cost.overrides);
    let finalization: Bound<'py, PyAny> =
        new_record_with(py, RecordId::BuildCostFinalization, &fields)?;
    finalize_cost(&finalization)
}

fn cost_status<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    host::entry(py, ExecutorHost::CostStatus)?.getattr(name)
}

/// Record a pending cost state before execution starts.
pub(crate) fn record_pending_cost<'py>(
    context: &BuildContext<'py>,
    pipeline: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.py();
    let started_at: Bound<'py, PyAny> = utc_now(py)?;
    finalize_build_cost(
        context,
        pipeline,
        CostRequest {
            status: "running",
            started_at: started_at.clone(),
            completed_at: started_at.clone(),
            overrides: vec![
                ("collect", boolean(py, false)),
                ("render", boolean(py, false)),
                ("cost_status", cost_status(py, "PENDING")?),
                ("cost_message", text(py, PENDING_MESSAGE)),
            ],
        },
    )?;
    Ok(started_at)
}

/// Retain cost collection elapsed time on the active timing tracker.
fn finish_cost_timing(context: &BuildContext<'_>, started: Instant) -> f64 {
    let elapsed: f64 = started.elapsed().as_secs_f64();
    let _ = context.tracker.setattr("cost_collection_seconds", elapsed);
    elapsed
}

fn style(context: &BuildContext<'_>) -> CliStyle {
    CliStyle {
        use_color: context.use_color,
    }
}

fn write_timings(context: &BuildContext<'_>, timings: &PhaseTimings) -> PyResult<()> {
    emit(
        &context.stream,
        &render_phase_timings(timings, style(context)),
    )
}

/// Write the phases the tracker measured before an exceptional exit, to stderr without colour.
pub(crate) fn write_partial_timings(py: Python<'_>, tracker: &Bound<'_, PyAny>) -> PyResult<()> {
    let snapshot: Bound<'_, PyAny> = tracker.call_method0("snapshot")?;
    let seconds =
        |name: &str| -> PyResult<Option<f64>> { native_field(&snapshot, name)?.extract() };
    let timings: PhaseTimings = PhaseTimings {
        compile_seconds: seconds("compile_seconds")?,
        planning_seconds: seconds("planning_seconds")?,
        connection_preparation_seconds: seconds("connection_preparation_seconds")?,
        schema_preparation_seconds: seconds("schema_preparation_seconds")?,
        execution_seconds: seconds("execution_seconds")?,
        cost_collection_seconds: seconds("cost_collection_seconds")?,
        total_seconds: seconds("total_seconds")?,
    };
    emit(
        &py.import("sys")?.getattr("stderr")?,
        &render_phase_timings(&timings, CliStyle { use_color: false }),
    )
}

fn cost_payload<'py>(py: Python<'py>, record: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if record.is_none() {
        return Ok(none(py));
    }
    cost_output_payload(py, &record.clone())
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))
}

fn run_id_for<'py>(
    pipeline: &Bound<'py, PyAny>,
    record: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if record.is_none() {
        return Ok(none(pipeline.py()));
    }
    native_field(pipeline, "project")?.getattr("run_id")
}

fn write_json(context: &BuildContext<'_>, payload: Bound<'_, PyAny>) -> PyResult<()> {
    write_execution_json_output(
        context.py(),
        payload,
        native_field(&context.request, "json_output")?.extract()?,
        native_field(&context.request, "json_output_path")?,
    )
}

/// Persist and emit a completed zero-cost record when the plan has no work.
fn finalize_no_work(context: &BuildContext<'_>, pipeline: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py: Python<'_> = context.py();
    let plan: Bound<'_, PyAny> = native_field(pipeline, "plan_output")?;
    if plan_has_executable_work(
        py,
        Some(keywords(
            py,
            &[
                ("plan", plan.clone()),
                (
                    "python_plan_entries",
                    native_field(pipeline, "python_plan_entries")?,
                ),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
    .is_truthy()?
    {
        return Ok(false);
    }
    if context.verbose {
        write_run_context(context, pipeline)?;
    }
    let completed_at: Bound<'_, PyAny> = utc_now(py)?;
    let record: Bound<'_, PyAny> = finalize_build_cost(
        context,
        pipeline,
        CostRequest {
            status: SUCCESS_STATUS,
            started_at: completed_at.clone(),
            completed_at,
            overrides: vec![
                ("collect", boolean(py, false)),
                ("had_executable_work", boolean(py, false)),
            ],
        },
    )?;
    let result: Bound<'_, PyAny> = call_with(
        &host::entry(py, ExecutorHost::BuildExecutionResult)?,
        &[(
            "status",
            host::entry(py, ExecutorHost::BuildStatus)?.getattr("SUCCESS")?,
        )],
    )?;
    let payload: Bound<'_, PyAny> = format_build_execution_json(
        py,
        Some(keywords(
            py,
            &[
                ("result", result),
                ("plan", plan),
                ("run_id", run_id_for(pipeline, &record)?),
                ("cost", cost_payload(py, &record)?),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    write_json(context, payload)?;
    Ok(true)
}

/// Finalize a build without executable work, with verbose phase timings.
pub(crate) fn finalize_no_work_with_timings(
    context: &BuildContext<'_>,
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let cost_started: Instant = Instant::now();
    let finalized: PyResult<bool> = finalize_no_work(context, pipeline);
    let cost_seconds: f64 = finish_cost_timing(context, cost_started);
    let finalized: bool = finalized?;
    if finalized && context.verbose {
        write_timings(
            context,
            &PhaseTimings {
                compile_seconds: native_field(pipeline, "compile_seconds")?.extract()?,
                planning_seconds: native_field(pipeline, "planning_seconds")?.extract()?,
                cost_collection_seconds: Some(cost_seconds),
                total_seconds: Some(context.started.elapsed().as_secs_f64()),
                ..PhaseTimings::default()
            },
        )?;
    }
    Ok(finalized)
}

/// Persist failed or interrupted cost state without masking the active error.
pub(crate) fn finalize_exceptional_cost(
    context: &BuildContext<'_>,
    pipeline: &Bound<'_, PyAny>,
    (started_at, error): (&Bound<'_, PyAny>, &PyErr),
) {
    let py: Python<'_> = context.py();
    let cost_started: Instant = Instant::now();
    let interrupted: bool = error.is_instance_of::<PyKeyboardInterrupt>(py);
    let _ = (|| -> PyResult<Bound<'_, PyAny>> {
        finalize_build_cost(
            context,
            pipeline,
            CostRequest {
                status: if interrupted { "interrupted" } else { "failed" },
                started_at: started_at.clone(),
                completed_at: utc_now(py)?,
                overrides: vec![
                    ("collect", boolean(py, !interrupted)),
                    ("render", boolean(py, false)),
                    ("cost_status", cost_status(py, "PARTIAL")?),
                    ("cost_message", text(py, INTERRUPTED_MESSAGE)),
                ],
            },
        )
    })();
    let _ = finish_cost_timing(context, cost_started);
}

fn any_failed(
    results: &Bound<'_, PyAny>,
    failed: impl Fn(&Bound<'_, PyAny>) -> PyResult<bool>,
) -> PyResult<bool> {
    for result in results.try_iter()? {
        if failed(&result?)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A Python check result as the footer counts it: a failure fails the build, a warning warns.
fn check_node_text(check: &CheckResultText) -> PythonNodeText {
    let message: Option<String> = check
        .error_message
        .clone()
        .or_else(|| check.message.clone());
    PythonNodeText {
        name: check.name.clone(),
        kind: CHECK_KIND.to_owned(),
        status: if check.status == CheckStatus::Fail {
            "failed"
        } else {
            SUCCESS_STATUS
        }
        .to_owned(),
        warnings: if check.status == CheckStatus::Warn {
            vec![message.clone().unwrap_or_else(|| CHECK_WARNED.to_owned())]
        } else {
            Vec::new()
        },
        error_message: message,
    }
}

/// Persist and render a completed direct build and return its exit code.
pub(crate) fn complete_direct_build<'py>(
    context: &BuildContext<'py>,
    pipeline: &Bound<'py, PyAny>,
    (preparation, outcome, check_results): (&Preparation<'py>, &Outcome<'py>, &Bound<'py, PyAny>),
    started_at: &Bound<'py, PyAny>,
) -> PyResult<i32> {
    let py: Python<'py> = context.py();
    let completed_at: Bound<'py, PyAny> = utc_now(py)?;
    let result = &outcome.result;
    let plan: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    let succeeded: bool = result.status == BuildStatus::Success;
    let python_failed: bool = any_failed(&outcome.python_results, |result| {
        Ok(python_node_failed(
            &native_field(result, "status")?.str()?.extract::<String>()?,
        ))
    })?;
    let checks_failed: bool = any_failed(check_results, |result| {
        native_field(result, "failed")?.is_truthy()
    })?;
    let exit_code: i32 = build_exit_code(succeeded, (python_failed, checks_failed));
    let target_dir: Bound<'py, PyAny> =
        joined(&context.get("effective_project_dir")?, &["target"])?;
    write_runtime_target(&target_dir, &plan, result)?;
    write_python_check_runtime_target(&target_dir, check_results)?;
    let cost_started: Instant = Instant::now();
    let record: PyResult<Bound<'py, PyAny>> = finalize_build_cost(
        context,
        pipeline,
        CostRequest {
            status: if exit_code == 0 {
                SUCCESS_STATUS
            } else {
                "failed"
            },
            started_at: started_at.clone(),
            completed_at,
            overrides: vec![("render", boolean(py, false))],
        },
    );
    let cost_seconds: f64 = finish_cost_timing(context, cost_started);
    let record: Bound<'py, PyAny> = record?;
    let checks: Vec<PythonNodeText> = check_results
        .try_iter()?
        .map(|check| Ok(check_node_text(&check_result_text(&check?)?)))
        .collect::<PyResult<_>>()?;
    let footer: String = format_build_footer_with(
        &build_result_text(py, result)?,
        (
            native_field(&preparation.callbacks, "elapsed")?.extract::<f64>()?,
            context.use_color,
        ),
        Some(&outcome.python_results.clone()).filter(|value| !value.is_none()),
        checks,
    )?;
    emit(&context.stream, &format!("\n{footer}\n"))?;
    let payload: Bound<'py, PyAny> = format_build_execution_json_with(
        py,
        result,
        Some(keywords(
            py,
            &[
                ("plan", plan),
                ("python_node_results", outcome.python_results.clone()),
                ("python_check_results", check_results.clone()),
                ("command", text(py, "build")),
                ("run_id", run_id_for(pipeline, &record)?),
                ("cost", cost_payload(py, &record)?),
                ("execution_succeeded", boolean(py, exit_code == 0)),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    write_json(context, payload)?;
    render_build_cost(&record, &context.stream, context.use_color)?;
    if context.verbose {
        let timings = &result.timings;
        write_timings(
            context,
            &PhaseTimings {
                compile_seconds: native_field(pipeline, "compile_seconds")?.extract()?,
                planning_seconds: native_field(pipeline, "planning_seconds")?.extract()?,
                connection_preparation_seconds: timings.connection_preparation_seconds,
                schema_preparation_seconds: timings.schema_preparation_seconds,
                execution_seconds: timings.execution_seconds,
                cost_collection_seconds: Some(cost_seconds),
                total_seconds: Some(context.started.elapsed().as_secs_f64()),
            },
        )?;
    }
    Ok(exit_code)
}
