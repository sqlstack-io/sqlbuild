//! Execution-result JSON documents for build, seed, load, clone, test, audit and scenario commands.

use crate::result_output::main::results::audit_execution_json::audit_execution_json;
use crate::result_output::main::results::audit_projection::audit_projection;
use crate::result_output::main::results::test_execution_json::test_execution_json;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::{
    AuditExecutionResult, BuildExecutionResult, BuildStatus, ExecutionStatus,
    ScenarioLocalRunStatus, ScenarioRunResult, SqlTestExecutionResult,
};

use crate::result_output::_helpers::results::aggregation::RetainedBuildResults;
use crate::result_output::_helpers::results::assets::function_assets;
use crate::result_output::_helpers::results::assets::load_assets;
use crate::result_output::_helpers::results::assets::model_assets;
use crate::result_output::_helpers::results::assets::python_node_assets;
use crate::result_output::_helpers::results::assets::seed_assets;
use crate::result_output::_helpers::results::assets::without_none;
use crate::result_output::_helpers::results::checks::audit_checks;
use crate::result_output::_helpers::results::checks::python_checks;
use crate::result_output::_helpers::results::checks::scenario_check;
use crate::result_output::_helpers::results::checks::scenario_result;
use crate::result_output::_helpers::results::checks::sql_test_checks;
use crate::result_output::_helpers::results::projection::Projection;
use crate::result_output::_helpers::results::projection::rounded;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::compile_outputs::main::indented_json::indented_json;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::types::ReadResult;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_planner::plan_output::main::any::any;
use sqlbuild_python_planner::plan_output::main::dict::dict;
use sqlbuild_python_planner::plan_output::main::text::text;

const JSON_VERSION: i64 = 1;
pub(crate) const FAILED: &str = "failed";
const SUCCESS: &str = "success";
const SKIPPED: &str = "skipped";
pub(crate) const BUILD_OPTIONS: (&[&str], &[&str]) = (
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
pub(crate) const TYPED_BUILD_OPTIONS: (&[&str], &[&str]) =
    (&["plan", "execution_succeeded"], BUILD_OPTIONS.1);
pub(crate) const SCENARIO_OPTIONS: (&[&str], &[&str]) = (
    &["results"],
    &["local", "run_namespace", "namespace_source", "duration_ms"],
);
pub(crate) const SNAPSHOT_OPTIONS: (&[&str], &[&str]) = (
    &["results"],
    &["refresh", "run_namespace", "namespace_source"],
);
/// The optional sections of a document after `checks`.
#[derive(Default)]
pub(crate) struct Extras<'py> {
    pub(crate) scenarios: Vec<Bound<'py, PyDict>>,
    pub(crate) run_id: Option<Bound<'py, PyAny>>,
    pub(crate) cost: Option<Bound<'py, PyAny>>,
    pub(crate) execution: Option<Bound<'py, PyDict>>,
    /// Entries appended after every other section.
    pub(crate) trailing: Vec<(&'static str, Bound<'py, PyAny>)>,
}

/// The document body shared by every command.
pub(crate) struct Document<'py> {
    pub(crate) command: String,
    pub(crate) status: String,
    pub(crate) summary: Bound<'py, PyDict>,
    pub(crate) assets: Vec<Bound<'py, PyDict>>,
    pub(crate) checks: Vec<Bound<'py, PyDict>>,
}

/// `json.dumps(payload, indent=2) + "\n"`, failed when the lifecycle terminal failed.
pub(crate) fn emitted<'py>(
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

pub(crate) fn count_status(
    results: &[Bound<'_, PyAny>],
    field: &str,
    status: &str,
) -> PyResult<usize> {
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

pub(crate) fn summary<'py>(
    py: Python<'py>,
    entries: &[(&str, usize)],
) -> PyResult<Bound<'py, PyDict>> {
    dict(
        py,
        entries
            .iter()
            .map(|(key, count)| Ok((*key, any(py, *count)?)))
            .collect::<PyResult<_>>()?,
    )
}

pub(crate) fn outcome_status(fail_count: usize) -> String {
    if fail_count == 0 { SUCCESS } else { FAILED }.to_owned()
}

/// The argument when it was passed and is not `None`.
pub(crate) fn passed<'py>(
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

pub(crate) fn count(value: i64) -> PyResult<usize> {
    usize::try_from(value).map_err(|_| {
        pyo3::exceptions::PyOverflowError::new_err("can't convert negative int to unsigned")
    })
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

pub(crate) fn degradable_build_json<'py>(
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
            &host_entry(py, ExecutorHost::ObservabilityValidationError)?,
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

pub(crate) fn build_json<'py>(
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

pub(crate) fn namespace_execution<'py>(
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

pub(crate) fn scenario_json<'py>(
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
