//! Execution-result JSON documents for build, seed, load, clone, test, audit and scenario commands.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::{
    AuditExecutionResult, AuditOutcome, BuildExecutionResult, BuildStatus, ExecutionStatus,
    LoadExecutionResult, ScenarioLocalRunStatus, ScenarioRunResult, SeedExecutionResult,
    SqlTestExecutionResult, SqlTestOutcome,
};

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compile_outputs::json_report::indented_json;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::results::ReadResult;
use crate::bindings::_helpers::plan_output::values::{any, dict, items, text};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::result_output::aggregation::RetainedBuildResults;
use crate::bindings::_helpers::result_output::assets::{
    clone_asset, function_assets, load_assets, model_assets, python_node_assets, seed_assets,
    without_none,
};
use crate::bindings::_helpers::result_output::checks::{
    audit_checks, python_checks, scenario_check, scenario_result, snapshot_result, sql_test_checks,
};
use crate::bindings::_helpers::result_output::projection::{Projection, rounded};

const JSON_VERSION: i64 = 1;
const FAILED: &str = "failed";
const SUCCESS: &str = "success";
const SKIPPED: &str = "skipped";
const BUILD_OPTIONS: (&[&str], &[&str]) = (
    &["result", "plan"],
    &[
        "python_node_results",
        "python_check_results",
        "command",
        "run_id",
        "cost",
    ],
);
const DEGRADABLE_BUILD_OPTIONS: (&[&str], &[&str]) = (
    &["result", "plan", "execution_succeeded"],
    &[
        "python_node_results",
        "python_check_results",
        "command",
        "run_id",
        "cost",
    ],
);
const TYPED_BUILD_OPTIONS: (&[&str], &[&str]) = (&["plan", "execution_succeeded"], BUILD_OPTIONS.1);
const SCENARIO_OPTIONS: (&[&str], &[&str]) = (
    &["results"],
    &["local", "run_namespace", "namespace_source", "duration_ms"],
);
const SNAPSHOT_OPTIONS: (&[&str], &[&str]) = (
    &["results"],
    &["refresh", "run_namespace", "namespace_source"],
);
/// The optional sections of a document after `checks`.
#[derive(Default)]
struct Extras<'py> {
    scenarios: Vec<Bound<'py, PyDict>>,
    run_id: Option<Bound<'py, PyAny>>,
    cost: Option<Bound<'py, PyAny>>,
    execution: Option<Bound<'py, PyDict>>,
    /// Entries appended after every other section.
    trailing: Vec<(&'static str, Bound<'py, PyAny>)>,
}

/// The document body shared by every command.
struct Document<'py> {
    command: String,
    status: String,
    summary: Bound<'py, PyDict>,
    assets: Vec<Bound<'py, PyDict>>,
    checks: Vec<Bound<'py, PyDict>>,
}

/// `json.dumps(payload, indent=2) + "\n"`, failed when the lifecycle terminal failed.
fn emitted<'py>(
    projection: &Projection<'py>,
    document: Document<'py>,
    extras: Extras<'py>,
) -> PyResult<String> {
    let py: Python<'py> = document.summary.py();
    let mut status: String = document.status;
    if let Some(terminal) = projection.lifecycle_terminal()?
        && text(&native_field(&terminal, "event_type")?)?.ends_with(FAILED)
    {
        status = FAILED.to_owned();
    }
    let payload: Bound<'py, PyDict> = dict(
        py,
        vec![
            ("version", any(py, JSON_VERSION)?),
            ("command", any(py, document.command)?),
            ("status", any(py, status)?),
            ("summary", document.summary.into_any()),
            ("assets", PyTuple::new(py, document.assets)?.into_any()),
            ("checks", PyTuple::new(py, document.checks)?.into_any()),
        ],
    )?;
    if !extras.scenarios.is_empty() {
        payload.set_item("scenarios", PyTuple::new(py, extras.scenarios)?)?;
    }
    if let Some(run_id) = extras.run_id.filter(|run_id| !run_id.is_none()) {
        payload.set_item("run_id", run_id)?;
    }
    if let Some(cost) = extras.cost.filter(|cost| !cost.is_none()) {
        payload.set_item("cost", cost)?;
    }
    if let Some(execution) = extras.execution {
        payload.set_item("execution", execution)?;
    }
    for (key, value) in extras.trailing {
        payload.set_item(key, value)?;
    }
    Ok(indented_json(payload.as_any())? + "\n")
}

fn count_status(results: &[Bound<'_, PyAny>], field: &str, status: &str) -> PyResult<usize> {
    let mut count: usize = 0;
    for result in results {
        if native_field(&native_field(result, field)?, "value")?.eq(status)? {
            count += 1;
        }
    }
    Ok(count)
}

fn count_truthy(results: &[Bound<'_, PyAny>], field: &str) -> PyResult<usize> {
    let mut count: usize = 0;
    for result in results {
        if native_field(result, field)?.is_truthy()? {
            count += 1;
        }
    }
    Ok(count)
}

fn summary<'py>(py: Python<'py>, entries: &[(&str, usize)]) -> PyResult<Bound<'py, PyDict>> {
    dict(
        py,
        entries
            .iter()
            .map(|(key, count)| Ok((*key, any(py, *count)?)))
            .collect::<PyResult<_>>()?,
    )
}

fn outcome_status(fail_count: usize) -> String {
    if fail_count == 0 { SUCCESS } else { FAILED }.to_owned()
}

/// The argument when it was passed and is not `None`.
fn passed<'py>(
    arguments: &KeywordArguments<'py>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let value: Bound<'py, PyAny> = arguments.value(name)?;
    Ok((!value.is_none()).then_some(value))
}

/// A sequence argument that defaults to empty.
fn sequence<'py>(py: Python<'py>, value: Option<Bound<'py, PyAny>>) -> Bound<'py, PyAny> {
    value.unwrap_or_else(|| PyTuple::empty(py).into_any())
}

fn count(value: i64) -> PyResult<usize> {
    usize::try_from(value).map_err(|_| {
        pyo3::exceptions::PyOverflowError::new_err("can't convert negative int to unsigned")
    })
}

/// Format build command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn format_build_execution_json<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "format_build_execution_json", options, BUILD_OPTIONS)?;
    let result = BuildExecutionResult::read(&arguments.value("result")?)?;
    build_json(py, &arguments, &result, Vec::new())
}

/// The audit-history projection counts, flagged as degraded when any write failed.
pub(crate) fn audit_projection<'py>(
    py: Python<'py>,
    (attempted, written, failed): (i64, i64, i64),
) -> PyResult<Vec<(&'static str, Bound<'py, PyAny>)>> {
    let counts: Vec<(&str, Bound<'py, PyAny>)> = [
        ("attempted_count", attempted),
        ("written_count", written),
        ("failed_count", failed),
    ]
    .into_iter()
    .map(|(key, value)| Ok((key, any(py, value)?)))
    .collect::<PyResult<_>>()?;
    let failed: bool = counts[2].1.is_truthy()?;
    let mut entries: Vec<(&'static str, Bound<'py, PyAny>)> = vec![(
        "audit_result_projection",
        dict(py, counts.clone())?.into_any(),
    )];
    if failed {
        let mut reason: Vec<(&str, Bound<'py, PyAny>)> =
            vec![("reason", any(py, "audit_result_projection_failure")?)];
        reason.extend(counts);
        entries.push(("projection_degraded", any(py, true)?));
        entries.push((
            "projection_degradation_reasons",
            PyList::new(py, [dict(py, reason)?])?.into_any(),
        ));
    }
    Ok(entries)
}

/// The build document with its audit-history projection, or a minimal degraded document.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn format_degradable_build_execution_json<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "format_degradable_build_execution_json",
        options,
        DEGRADABLE_BUILD_OPTIONS,
    )?;
    let result = BuildExecutionResult::read(&arguments.value("result")?)?;
    degradable_build_json(py, &arguments, &result)
}

/// Production build completion already owns the converted executor result.
pub(crate) fn format_build_execution_json_with<'py>(
    py: Python<'py>,
    result: &BuildExecutionResult<Bound<'py, PyAny>>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments = KeywordArguments::read(
        py,
        "format_build_execution_json",
        options,
        TYPED_BUILD_OPTIONS,
    )?;
    degradable_build_json(py, &arguments, result)
}

fn degradable_build_json<'py>(
    py: Python<'py>,
    arguments: &KeywordArguments<'py>,
    result: &BuildExecutionResult<Bound<'py, PyAny>>,
) -> PyResult<String> {
    let execution_succeeded: bool = arguments.flag("execution_succeeded")?;
    let rendered: PyResult<String> = audit_projection(
        py,
        (
            result.audit_result_projection_attempted_count,
            result.audit_result_projection_written_count,
            result.audit_result_projection_failed_count,
        ),
    )
    .and_then(|trailing| build_json(py, arguments, result, trailing));
    let error: pyo3::PyErr = match rendered {
        Ok(payload) => return Ok(payload),
        Err(error) => error,
    };
    let degradable: bool = error.is_instance_of::<pyo3::exceptions::PyTypeError>(py)
        || error.is_instance_of::<pyo3::exceptions::PyValueError>(py)
        || error.is_instance(
            py,
            &host::entry(py, ExecutorHost::ObservabilityValidationError)?,
        );
    if !degradable {
        return Err(error);
    }
    let command: String = if arguments.passed("command")? {
        arguments.value("command")?.extract()?
    } else {
        "build".to_owned()
    };
    degraded_document(
        py,
        (command, if execution_succeeded { SUCCESS } else { FAILED }),
        error.value(py).as_any(),
        execution_succeeded,
    )
}

fn build_json<'py>(
    py: Python<'py>,
    arguments: &KeywordArguments<'py>,
    result: &BuildExecutionResult<Bound<'py, PyAny>>,
    trailing: Vec<(&'static str, Bound<'py, PyAny>)>,
) -> PyResult<String> {
    let plan: Bound<'py, PyAny> = arguments.value("plan")?;
    let command: String = if arguments.passed("command")? {
        arguments.value("command")?.extract()?
    } else {
        "build".to_owned()
    };
    let projection: Projection<'py> = Projection::installed(py)?;
    let python_node_results: Bound<'py, PyAny> =
        sequence(py, passed(arguments, "python_node_results")?);
    let python_check_results: Bound<'py, PyAny> =
        sequence(py, passed(arguments, "python_check_results")?);
    let retained = RetainedBuildResults::new(result, &projection)?;
    let nodes: Vec<Bound<'py, PyAny>> = projection.terminal_results(&python_node_results)?;
    let checks_run: Vec<Bound<'py, PyAny>> = projection.terminal_results(&python_check_results)?;
    let partial = retained.is_partial(result)
        || nodes.len() != python_node_results.len()?
        || checks_run.len() != python_check_results.len()?;
    let python_fail: usize = count_status(&nodes, "status", FAILED)?;
    let python_success: usize = count_status(&nodes, "status", SUCCESS)?;
    let python_skipped: usize = count_status(&nodes, "status", SKIPPED)?;
    let check_fail: usize = count_truthy(&checks_run, "failed")?;
    let check_warn: usize = count_truthy(&checks_run, "warned")?;
    let check_pass: usize = count_truthy(&checks_run, "passed")?;
    let mut python_warnings: usize = 0;
    for warned in nodes.iter().chain(&checks_run) {
        python_warnings += native_field(warned, "warning_messages")?.len()?;
    }
    python_warnings += retained
        .loads
        .iter()
        .map(|result| result.warning_messages.len())
        .sum::<usize>();
    let status: String =
        if result.status == BuildStatus::Failed || python_fail > 0 || check_fail > 0 {
            FAILED.to_owned()
        } else {
            SUCCESS.to_owned()
        };
    let mut checks: Vec<Bound<'py, PyDict>> = sql_test_checks(&projection, &retained.tests)?;
    for audits in [&retained.source_audits, &retained.end_audits] {
        checks.extend(audit_checks(&projection, audits, false)?);
    }
    for model in &retained.models {
        checks.extend(audit_checks(
            &projection,
            &model.audit_results.iter().collect::<Vec<_>>(),
            projection.terminal_recorded(model)?,
        )?);
    }
    checks.extend(python_checks(&projection, &checks_run)?);
    let mut assets: Vec<Bound<'py, PyDict>> =
        model_assets(&projection, &retained.models, Some(&plan))?;
    assets.extend(seed_assets(&projection, &retained.seeds, Some(&plan))?);
    assets.extend(function_assets(
        &projection,
        &retained.functions,
        Some(&plan),
    )?);
    assets.extend(load_assets(&projection, &retained.loads)?);
    assets.extend(python_node_assets(&projection, &nodes)?);
    let counted: [usize; 5] = if partial {
        let counts = retained.counts();
        [
            counts.success,
            counts.failure,
            counts.timeout,
            counts.skipped,
            counts.warning,
        ]
    } else {
        [
            count(result.success_count)?,
            count(result.failure_count)?,
            count(result.timeout_count)?,
            count(result.skipped_count)?,
            count(result.warning_count)?,
        ]
    };
    let summary: Bound<'py, PyDict> = summary(
        py,
        &[
            ("success_count", counted[0] + python_success),
            ("failure_count", counted[1] + python_fail + check_fail),
            ("timeout_count", counted[2]),
            ("skipped_count", counted[3] + python_skipped),
            ("warning_count", counted[4] + check_warn + python_warnings),
            ("python_check_pass_count", check_pass),
            ("python_check_warn_count", check_warn),
            ("python_check_fail_count", check_fail),
        ],
    )?;
    emitted(
        &projection,
        Document {
            command,
            status,
            summary,
            assets,
            checks,
        },
        Extras {
            run_id: passed(arguments, "run_id")?,
            cost: passed(arguments, "cost")?,
            trailing,
            ..Extras::default()
        },
    )
}

/// Format seed command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (*, results, plan))]
pub(crate) fn format_seed_execution_json<'py>(
    py: Python<'py>,
    results: Bound<'py, PyAny>,
    plan: Bound<'py, PyAny>,
) -> PyResult<String> {
    let projection: Projection<'py> = Projection::installed(py)?;
    let all = Vec::<SeedExecutionResult>::read(&results)?;
    let results = projection.retained(&all)?;
    let fail_count = results
        .iter()
        .filter(|result| result.status == ExecutionStatus::Failed)
        .count();
    emitted(
        &projection,
        Document {
            command: "seed".to_owned(),
            status: outcome_status(fail_count),
            summary: summary(
                py,
                &[
                    (
                        "success_count",
                        results
                            .iter()
                            .filter(|result| result.status == ExecutionStatus::Success)
                            .count(),
                    ),
                    ("failure_count", fail_count),
                    ("total_count", results.len()),
                ],
            )?,
            assets: seed_assets(&projection, &results, Some(&plan))?,
            checks: Vec::new(),
        },
        Extras::default(),
    )
}

/// Format load command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (*, results))]
pub(crate) fn format_load_execution_json<'py>(
    py: Python<'py>,
    results: Bound<'py, PyAny>,
) -> PyResult<String> {
    let projection: Projection<'py> = Projection::installed(py)?;
    let all = Vec::<LoadExecutionResult<_>>::read(&results)?;
    let results = projection.retained(&all)?;
    let fail_count = results
        .iter()
        .filter(|result| result.status == ExecutionStatus::Failed)
        .count();
    emitted(
        &projection,
        Document {
            command: "load".to_owned(),
            status: outcome_status(fail_count),
            summary: summary(
                py,
                &[
                    (
                        "success_count",
                        results
                            .iter()
                            .filter(|result| result.status == ExecutionStatus::Success)
                            .count(),
                    ),
                    ("failure_count", fail_count),
                    (
                        "skipped_count",
                        results
                            .iter()
                            .filter(|result| result.status == ExecutionStatus::Skipped)
                            .count(),
                    ),
                    ("total_count", results.len()),
                ],
            )?,
            assets: load_assets(&projection, &results)?,
            checks: Vec::new(),
        },
        Extras::default(),
    )
}

/// Format direct clone command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (*, result, resource_types_by_name, phase_timings = None))]
pub(crate) fn format_clone_execution_json<'py>(
    py: Python<'py>,
    result: Bound<'py, PyAny>,
    resource_types_by_name: Bound<'py, PyAny>,
    phase_timings: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let projection: Projection<'py> = Projection::installed(py)?;
    let mut assets: Vec<Bound<'py, PyDict>> = Vec::new();
    let (mut failure_count, mut warning_count): (usize, usize) = (0, 0);
    for item in items(&native_field(&result, "item_results")?)? {
        let name: String = text(&native_field(&item, "name")?)?;
        let resource_type: String = text(&resource_types_by_name.get_item(&name)?)?;
        if !projection.resource_terminal_recorded(&name, &format!("{resource_type}:{name}"))? {
            continue;
        }
        let status: Bound<'py, PyAny> = native_field(&native_field(&item, "status")?, "value")?;
        failure_count += usize::from(status.eq(FAILED)?);
        warning_count += usize::from(status.eq("warning")?);
        assets.push(clone_asset(&projection, &item, &resource_type)?);
    }
    let total: usize = assets.len();
    emitted(
        &projection,
        Document {
            command: "clone".to_owned(),
            status: outcome_status(failure_count),
            summary: summary(
                py,
                &[
                    ("success_count", total - failure_count - warning_count),
                    ("failure_count", failure_count),
                    ("warning_count", warning_count),
                    ("total_count", total),
                ],
            )?,
            assets,
            checks: Vec::new(),
        },
        Extras {
            trailing: phase_timings
                .into_iter()
                .map(|timings| ("phase_timings", timings.into_any()))
                .collect(),
            ..Extras::default()
        },
    )
}

/// Format test command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (*, results))]
pub(crate) fn format_test_execution_json<'py>(
    py: Python<'py>,
    results: Bound<'py, PyAny>,
) -> PyResult<String> {
    let all = Vec::<SqlTestExecutionResult<_>>::read(&results)?;
    test_execution_json(py, &all)
}

/// The test command's document from the executor's already-converted results.
pub(crate) fn test_execution_json<'py>(
    py: Python<'py>,
    all: &[SqlTestExecutionResult<Bound<'py, PyAny>>],
) -> PyResult<String> {
    let projection: Projection<'py> = Projection::installed(py)?;
    let results = projection.retained(all)?;
    let pass_count = results
        .iter()
        .filter(|result| result.outcome == SqlTestOutcome::Pass)
        .count();
    let timeout_count = results
        .iter()
        .filter(|result| result.outcome == SqlTestOutcome::Timeout)
        .count();
    let fail_count: usize = results.len() - pass_count;
    emitted(
        &projection,
        Document {
            command: "test".to_owned(),
            status: outcome_status(fail_count),
            summary: summary(
                py,
                &[
                    ("pass_count", pass_count),
                    ("fail_count", fail_count),
                    ("timeout_count", timeout_count),
                    ("total_count", results.len()),
                ],
            )?,
            assets: Vec::new(),
            checks: sql_test_checks(&projection, &results)?,
        },
        Extras::default(),
    )
}

/// Format audit command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (*, results, configured_concurrency = None, worker_count = None))]
pub(crate) fn format_audit_execution_json<'py>(
    py: Python<'py>,
    results: Bound<'py, PyAny>,
    configured_concurrency: Option<Bound<'py, PyAny>>,
    worker_count: Option<Bound<'py, PyAny>>,
) -> PyResult<String> {
    let all = Vec::<AuditExecutionResult<_>>::read(&results)?;
    audit_execution_json(py, &all, (configured_concurrency, worker_count), Vec::new())
}

/// Audit reporting from already-converted executor results, before the document is serialized.
pub(crate) fn audit_execution_json<'py>(
    py: Python<'py>,
    all: &[AuditExecutionResult<Bound<'py, PyAny>>],
    (configured_concurrency, worker_count): (Option<Bound<'py, PyAny>>, Option<Bound<'py, PyAny>>),
    trailing: Vec<(&'static str, Bound<'py, PyAny>)>,
) -> PyResult<String> {
    let configured_concurrency: Bound<'py, PyAny> =
        configured_concurrency.map_or_else(|| any(py, 1_i64), Ok)?;
    let projection: Projection<'py> = Projection::installed(py)?;
    let results = projection.retained(all)?;
    let fail_count = results
        .iter()
        .filter(|result| result.outcome == AuditOutcome::Error)
        .count();
    let workers: Bound<'py, PyAny> = match worker_count.filter(|count| !count.is_none()) {
        Some(workers) => workers,
        None => py
            .import("builtins")?
            .getattr("min")?
            .call1((&configured_concurrency, results.len()))?,
    };
    emitted(
        &projection,
        Document {
            command: "audit".to_owned(),
            status: outcome_status(fail_count),
            summary: summary(
                py,
                &[
                    (
                        "pass_count",
                        results
                            .iter()
                            .filter(|result| result.outcome == AuditOutcome::Pass)
                            .count(),
                    ),
                    (
                        "warn_count",
                        results
                            .iter()
                            .filter(|result| result.outcome == AuditOutcome::Warn)
                            .count(),
                    ),
                    ("fail_count", fail_count),
                    ("total_count", results.len()),
                ],
            )?,
            assets: Vec::new(),
            checks: audit_checks(&projection, &results, false)?,
        },
        Extras {
            execution: Some(dict(
                py,
                vec![
                    ("configured_concurrency", configured_concurrency),
                    ("worker_count", workers),
                ],
            )?),
            trailing,
            ..Extras::default()
        },
    )
}

fn namespace_execution<'py>(
    py: Python<'py>,
    run_namespace: Option<Bound<'py, PyAny>>,
    namespace_source: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    dict(
        py,
        vec![
            (
                "scenario_namespace",
                run_namespace.unwrap_or_else(|| py.None().into_bound(py)),
            ),
            ("scenario_namespace_source", namespace_source),
        ],
    )
}

fn scenario_failed(result: &ScenarioRunResult<Bound<'_, PyAny>>, local: bool) -> bool {
    if !local {
        return result.status == ExecutionStatus::Failed;
    }
    matches!(
        result.local_status,
        Some(ScenarioLocalRunStatus::Fail | ScenarioLocalRunStatus::Error)
    )
}

/// Format scenario test command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn format_scenario_execution_json<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "format_scenario_execution_json",
        options,
        SCENARIO_OPTIONS,
    )?;
    let results = Vec::<ScenarioRunResult<_>>::read(&arguments.value("results")?)?;
    scenario_json(py, &arguments, &results)
}

/// Production scenario completion already owns its converted executor results.
pub(crate) fn format_scenario_execution_json_with<'py>(
    py: Python<'py>,
    results: &[ScenarioRunResult<Bound<'py, PyAny>>],
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments = KeywordArguments::read(
        py,
        "format_scenario_execution_json",
        options,
        (&[], SCENARIO_OPTIONS.1),
    )?;
    scenario_json(py, &arguments, results)
}

fn scenario_json<'py>(
    py: Python<'py>,
    arguments: &KeywordArguments<'py>,
    results: &[ScenarioRunResult<Bound<'py, PyAny>>],
) -> PyResult<String> {
    let local: bool = arguments.passed("local")? && arguments.flag("local")?;
    let run_namespace: Option<Bound<'py, PyAny>> = passed(arguments, "run_namespace")?;
    let namespace_source: Option<Bound<'py, PyAny>> = passed(arguments, "namespace_source")?;
    let duration_ms: Option<f64> = arguments.value("duration_ms")?.extract()?;
    let mut projection: Projection<'py> = Projection::installed(py)?;
    let mut kept: Vec<&ScenarioRunResult<Bound<'py, PyAny>>> = Vec::new();
    for result in results {
        let name = &result.scenario_name;
        if projection.resource_terminal_recorded(name, &format!("sql_scenario:{name}"))? {
            kept.push(result);
        }
    }
    let mut fail_count: usize = 0;
    let (mut assets, mut checks, mut scenarios) = (Vec::new(), Vec::new(), Vec::new());
    for result in &kept {
        fail_count += usize::from(scenario_failed(result, local));
        projection.scenario = Some(result.scenario_name.clone());
        assets.extend(seed_assets(
            &projection,
            &result.seed_results.iter().collect::<Vec<_>>(),
            None,
        )?);
        assets.extend(function_assets(
            &projection,
            &result.function_results.iter().collect::<Vec<_>>(),
            None,
        )?);
        assets.extend(model_assets(
            &projection,
            &result.model_results.iter().collect::<Vec<_>>(),
            None,
        )?);
        projection.scenario = None;
        checks.push(scenario_check(py, result)?);
        scenarios.push(scenario_result(py, result)?);
    }
    let mut summary: Bound<'py, PyDict> = summary(
        py,
        &[
            ("pass_count", kept.len() - fail_count),
            ("fail_count", fail_count),
            ("total_count", kept.len()),
        ],
    )?;
    if let Some(duration) = duration_ms {
        summary = without_none(
            py,
            vec![
                ("pass_count", any(py, kept.len() - fail_count)?),
                ("fail_count", any(py, fail_count)?),
                ("total_count", any(py, kept.len())?),
                ("duration_ms", rounded(py, duration)?),
            ],
        )?;
    }
    emitted(
        &projection,
        Document {
            command: "scenario test".to_owned(),
            status: outcome_status(fail_count),
            summary,
            assets,
            checks,
        },
        Extras {
            scenarios,
            execution: Some(namespace_execution(
                py,
                run_namespace,
                namespace_source.map_or_else(|| any(py, "unset"), Ok)?,
            )?),
            ..Extras::default()
        },
    )
}

/// Format scenario snapshot sync/refresh execution results as JSON.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn format_scenario_snapshot_execution_json<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "format_scenario_snapshot_execution_json",
        options,
        SNAPSHOT_OPTIONS,
    )?;
    let results: Bound<'py, PyAny> = arguments.value("results")?;
    let refresh: bool = arguments.passed("refresh")? && arguments.flag("refresh")?;
    let run_namespace: Option<Bound<'py, PyAny>> = passed(&arguments, "run_namespace")?;
    let namespace_source: Option<Bound<'py, PyAny>> = passed(&arguments, "namespace_source")?;
    let projection: Projection<'py> = Projection::installed(py)?;
    let results: Vec<Bound<'py, PyAny>> = items(&results)?;
    let fail_count: usize = count_status(&results, "status", FAILED)?;
    emitted(
        &projection,
        Document {
            command: if refresh {
                "scenario snapshot refresh"
            } else {
                "scenario snapshot sync"
            }
            .to_owned(),
            status: outcome_status(fail_count),
            summary: summary(
                py,
                &[
                    ("pass_count", results.len() - fail_count),
                    ("fail_count", fail_count),
                    ("total_count", results.len()),
                ],
            )?,
            assets: Vec::new(),
            checks: Vec::new(),
        },
        Extras {
            scenarios: results
                .iter()
                .map(snapshot_result)
                .collect::<PyResult<_>>()?,
            execution: Some(namespace_execution(
                py,
                run_namespace,
                namespace_source.map_or_else(|| any(py, "unset"), Ok)?,
            )?),
            ..Extras::default()
        },
    )
}

/// Warn about a failed result projection, then format a minimal valid document.
fn degraded_document(
    py: Python<'_>,
    (command, status): (String, &str),
    error: &Bound<'_, PyAny>,
    execution_succeeded: bool,
) -> PyResult<String> {
    let completion: &str = if execution_succeeded {
        "warehouse execution completed successfully"
    } else {
        "execution result remains authoritative"
    };
    let stderr: Bound<'_, PyAny> = py.import("sys")?.getattr("stderr")?;
    stderr.call_method1(
        "write",
        (format!(
            "warning: integration output degraded: {}; {completion}\n",
            error.str()?
        ),),
    )?;
    stderr.call_method0("flush")?;
    let payload: Bound<'_, PyDict> = dict(
        py,
        vec![
            ("version", any(py, JSON_VERSION)?),
            ("command", any(py, command)?),
            ("status", any(py, status)?),
            ("summary", PyDict::new(py).into_any()),
            ("assets", PyList::empty(py).into_any()),
            ("checks", PyList::empty(py).into_any()),
            ("projection_degraded", any(py, true)?),
        ],
    )?;
    Ok(indented_json(payload.as_any())? + "\n")
}

/// Write execution JSON to stdout or a requested side-channel file.
#[pyfunction]
#[pyo3(signature = (*, payload, json_output, json_output_path))]
pub(crate) fn write_execution_json_output(
    py: Python<'_>,
    payload: Bound<'_, PyAny>,
    json_output: bool,
    json_output_path: Bound<'_, PyAny>,
) -> PyResult<()> {
    if !json_output_path.is_none() {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("parents", true)?;
        keywords.set_item("exist_ok", true)?;
        native_field(&json_output_path, "parent")?.call_method("mkdir", (), Some(&keywords))?;
        let encoding: Bound<'_, PyDict> = PyDict::new(py);
        encoding.set_item("encoding", "utf-8")?;
        json_output_path.call_method("write_text", (payload,), Some(&encoding))?;
        return Ok(());
    }
    if json_output {
        let stdout: Bound<'_, PyAny> = py.import("sys")?.getattr("stdout")?;
        stdout.call_method1("write", (payload,))?;
        stdout.call_method0("flush")?;
    }
    Ok(())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_build_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_seed_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_load_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_clone_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_test_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_audit_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_scenario_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(
        format_scenario_snapshot_execution_json,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        format_degradable_build_execution_json,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(write_execution_json_output, module)?)
}
