//! The `checks` and `scenarios` entries of execution-result documents.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};
use sqlbuild_core::json::models::JsonValue;
use sqlbuild_host::models::{
    AuditEvaluationMode, AuditExecutionResult, AuditOutcome, ExecutionStatus, MeasuredValue,
    ScenarioRunResult, SqlTestDifferenceSample, SqlTestExecutionResult, SqlTestOutcome, StepResult,
};
use sqlbuild_host::types::ResultEnum;

use crate::bindings::_helpers::plan_output::values::{any, dict, items, text};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::result_output::assets::{or_none, without_none};
use crate::bindings::_helpers::result_output::projection::{Projection, ResultIdentity, rounded};
use crate::bindings::_helpers::scopes::scope_report_objects::json_to_python;

const DECIMAL_KIND: &str = "decimal";
const SCALAR_KINDS: [&str; 5] = ["string", "integer", "boolean", "float", "null"];
const COLLECTION_KINDS: [&str; 2] = ["list", "set"];
const ERROR_FIELDS: [&str; 3] = ["error_code", "error_help", "error_message"];
const COMPARISON_COMPLETED: &str = "completed";
const COMPARISON_NOT_RUN: &str = "not_run";

fn value<'py>(result: &Bound<'py, PyAny>, field: &str) -> PyResult<Bound<'py, PyAny>> {
    native_field(&native_field(result, field)?, "value")
}

fn fields<'py>(
    result: &Bound<'py, PyAny>,
    names: &[&'static str],
) -> PyResult<Vec<(&'static str, Bound<'py, PyAny>)>> {
    names
        .iter()
        .map(|name| Ok((*name, native_field(result, name)?)))
        .collect()
}

fn tuple_of<'py>(py: Python<'py>, entries: Vec<Bound<'py, PyDict>>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, entries)?.into_any())
}

/// A SQL value as plain JSON: decimals as text, collections as lists, structs as objects.
fn sql_value_json<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let kind: String = text(&native_field(&native_field(value, "kind")?, "value")?)?;
    let inner: Bound<'py, PyAny> = native_field(value, "value")?;
    if kind == DECIMAL_KIND {
        return Ok(inner.str()?.into_any());
    }
    if SCALAR_KINDS.contains(&kind.as_str()) {
        return Ok(inner);
    }
    if COLLECTION_KINDS.contains(&kind.as_str()) {
        let list: Bound<'py, PyList> = PyList::empty(py);
        for item in items(&inner)? {
            list.append(sql_value_json(&item)?)?;
        }
        return Ok(list.into_any());
    }
    let object: Bound<'py, PyDict> = PyDict::new(py);
    for item in items(&inner)? {
        let (name, item): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item.extract()?;
        object.set_item(name, sql_value_json(&item)?)?;
    }
    Ok(object.into_any())
}

/// The case, schema and parameter values of a parameterized SQL test.
fn case_metadata<'py>(
    py: Python<'py>,
    result: &SqlTestExecutionResult<Bound<'py, PyAny>>,
) -> PyResult<Vec<(&'static str, Bound<'py, PyAny>)>> {
    let Some(source_path) = result
        .source_path
        .as_ref()
        .filter(|_| result.case_name.is_some())
    else {
        return Ok(Vec::new());
    };
    let parameter_types: Bound<'py, PyDict> = PyDict::new(py);
    let mut schema: Vec<Bound<'py, PyDict>> = Vec::new();
    for parameter in &result.parameter_schema {
        let parameter_type: Bound<'py, PyAny> = value(parameter, "value_type")?;
        parameter_types.set_item(native_field(parameter, "name")?, &parameter_type)?;
        schema.push(dict(
            py,
            vec![
                ("name", native_field(parameter, "name")?),
                ("type", parameter_type),
                ("nullable", native_field(parameter, "nullable")?),
            ],
        )?);
    }
    let mut parameters: Vec<Bound<'py, PyDict>> = Vec::new();
    for (name, parameter_value) in &result.parameter_values {
        let parameter_type: Bound<'py, PyAny> = parameter_types.as_any().get_item(name)?;
        parameters.push(dict(
            py,
            vec![
                ("name", any(py, name)?),
                ("type", parameter_type),
                ("value", sql_value_json(parameter_value)?),
            ],
        )?);
    }
    let mut metadata: Vec<(&'static str, Bound<'py, PyAny>)> = vec![
        (
            "source_path",
            any(
                py,
                source_path
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            )?,
        ),
        ("block_index", any(py, result.block_index)?),
        ("parent_name", any(py, &result.parent_name)?),
        ("case_name", any(py, &result.case_name)?),
        ("case_index", any(py, result.case_index)?),
        ("case_fingerprint", any(py, &result.case_fingerprint)?),
    ];
    metadata.push(("parameter_schema", tuple_of(py, schema)?));
    metadata.push(("parameters", tuple_of(py, parameters)?));
    Ok(metadata)
}

fn difference_samples<'py>(
    py: Python<'py>,
    samples: &[SqlTestDifferenceSample],
) -> PyResult<Bound<'py, PyAny>> {
    let formatted: Bound<'py, PyList> = PyList::empty(py);
    for sample in samples {
        let pairs: Bound<'py, PyList> = PyList::empty(py);
        for (name, sample_value) in &sample.values {
            pairs.append(dict(
                py,
                vec![("name", any(py, name)?), ("value", any(py, sample_value)?)],
            )?)?;
        }
        formatted.append(pairs)?;
    }
    or_none(formatted.into_any())
}

fn sql_test_step<'py>(py: Python<'py>, step: &StepResult) -> PyResult<Bound<'py, PyDict>> {
    let differences: Bound<'py, PyList> = PyList::empty(py);
    for difference in &step.column_differences {
        differences.append(dict(
            py,
            vec![
                ("name", any(py, &difference.name)?),
                ("actual", any(py, &difference.actual)?),
                ("expected", any(py, &difference.expected)?),
            ],
        )?)?;
    }
    let mut entries: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("model_name", any(py, &step.model_name)?),
        ("status", any(py, step.outcome.value())?),
        ("actual_row_count", any(py, step.actual_row_count)?),
        ("expected_row_count", any(py, step.expected_row_count)?),
        ("mismatched_row_count", any(py, step.mismatched_row_count)?),
        ("unexpected_row_count", any(py, step.unexpected_row_count)?),
        ("missing_row_count", any(py, step.missing_row_count)?),
    ];
    entries.push((
        "unexpected_samples",
        difference_samples(py, &step.unexpected_samples)?,
    ));
    entries.push((
        "missing_samples",
        difference_samples(py, &step.missing_samples)?,
    ));
    entries.push(("column_differences", or_none(differences.into_any())?));
    entries.extend([
        ("error_code", any(py, &step.error_code)?),
        ("error_help", any(py, &step.error_help)?),
        ("error_message", any(py, &step.error_message)?),
    ]);
    without_none(py, entries)
}

/// SQL test checks with their steps and parameterized case metadata.
pub(crate) fn sql_test_checks<'py>(
    projection: &Projection<'py>,
    results: &[&SqlTestExecutionResult<Bound<'py, PyAny>>],
) -> PyResult<Vec<Bound<'py, PyDict>>> {
    let mut checks: Vec<Bound<'py, PyDict>> = Vec::new();
    for result in results {
        if !projection.terminal_recorded(result)? {
            continue;
        }
        let py: Python<'py> = projection.py;
        let steps = &result.step_results;
        let outcome = result.outcome;
        let mut entries: Vec<(&str, Bound<'py, PyAny>)> = vec![
            ("kind", any(py, "sql_test")?),
            ("name", any(py, &result.test_name)?),
            ("check_id", any(py, result.identity()?.map(|(_, id)| id))?),
        ];
        entries.extend(case_metadata(py, result)?);
        entries.push(("passed", any(py, outcome == SqlTestOutcome::Pass)?));
        entries.push(("status", any(py, outcome.value())?));
        entries.push((
            "duration_ms",
            projection.result_duration(result, &py.None().into_bound(py))?,
        ));
        entries.push((
            "asset_name",
            match steps.first() {
                Some(step) => any(py, &step.model_name)?,
                None => py.None().into_bound(py),
            },
        ));
        entries.extend([
            ("error_code", any(py, &result.error_code)?),
            ("error_help", any(py, &result.error_help)?),
            ("error_message", any(py, &result.error_message)?),
        ]);
        entries.push((
            "steps",
            tuple_of(
                py,
                steps
                    .iter()
                    .map(|step| sql_test_step(py, step))
                    .collect::<PyResult<_>>()?,
            )?,
        ));
        checks.push(without_none(py, entries)?);
    }
    Ok(checks)
}

fn threshold_bound<'py>(bound: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = bound.py();
    let rendered: Bound<'py, PyDict> = dict(py, vec![("operator", value(bound, "operator")?)])?;
    let limit: Bound<'py, PyAny> = native_field(bound, "limit")?;
    if limit.is_none() {
        rendered.set_item("lower", native_field(bound, "lower")?)?;
        rendered.set_item("upper", native_field(bound, "upper")?)?;
    } else {
        rendered.set_item("limit", limit)?;
    }
    Ok(rendered.into_any())
}

/// The warn and error bounds of measurement thresholds, or `None`.
pub(crate) fn measurement_thresholds<'py>(
    thresholds: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if thresholds.is_none() {
        return Ok(thresholds.clone());
    }
    let rendered: Bound<'py, PyDict> = PyDict::new(thresholds.py());
    for severity in ["warn", "error"] {
        let bound: Bound<'py, PyAny> = native_field(thresholds, severity)?;
        if !bound.is_none() {
            rendered.set_item(severity, threshold_bound(&bound)?)?;
        }
    }
    Ok(rendered.into_any())
}

/// Keep integer measurements exact across both JSON output contracts.
pub(crate) fn measured_value<'py>(
    py: Python<'py>,
    value: Option<&MeasuredValue>,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Some(MeasuredValue::Integer(value)) => {
            Ok(json_to_python(py, JsonValue::Integer(value.clone()), false)?.into_bound(py))
        }
        Some(MeasuredValue::Float(value)) => any(py, *value),
        None => Ok(py.None().into_bound(py)),
    }
}

/// Audit checks; `terminal_evidence` keeps results whose model already has a terminal.
pub(crate) fn audit_checks<'py>(
    projection: &Projection<'py>,
    results: &[&AuditExecutionResult<Bound<'py, PyAny>>],
    terminal_evidence: bool,
) -> PyResult<Vec<Bound<'py, PyDict>>> {
    let mut checks: Vec<Bound<'py, PyDict>> = Vec::new();
    for result in results {
        if !terminal_evidence && !projection.terminal_recorded(&result)? {
            continue;
        }
        let py: Python<'py> = projection.py;
        let measurement = result.evaluation_mode == AuditEvaluationMode::Measurement;
        let measured = measured_value(py, result.measured_value.as_ref())?;
        let meta = match &result.audit_meta {
            Some(meta) => dict(
                py,
                meta.iter()
                    .map(|(key, value)| (key.as_str(), value.clone()))
                    .collect(),
            )?
            .into_any(),
            None => py.None().into_bound(py),
        };
        let mut entries: Vec<(&str, Bound<'py, PyAny>)> = vec![("kind", any(py, "audit")?)];
        entries.push(("name", any(py, &result.audit_name)?));
        entries.extend([
            (
                "audit_definition_name",
                any(py, &result.audit_definition_name)?,
            ),
            ("audit_description", any(py, &result.audit_description)?),
            ("audit_meta", meta),
        ]);
        entries.push(("check_id", any(py, result.identity()?.map(|(_, id)| id))?));
        entries.push((
            "passed",
            any(
                py,
                matches!(
                    result.outcome,
                    AuditOutcome::Pass | AuditOutcome::Insufficient
                ),
            )?,
        ));
        entries.push(("status", any(py, result.outcome.value())?));
        entries.push(("severity", any(py, result.severity.value())?));
        entries.push(("row_count", any(py, result.row_count)?));
        entries.push(("attachment_kind", any(py, result.attachment_kind.value())?));
        entries.push((
            "attached_target_kind",
            any(py, result.attached_target_kind.map(ResultEnum::value))?,
        ));
        entries.push(("asset_name", any(py, &result.attached_target_name)?));
        entries.push((
            "attached_column_name",
            any(py, &result.attached_column_name)?,
        ));
        entries.push(("run_scope_phase", any(py, result.run_scope_phase.value())?));
        entries.push(("reused", any(py, result.reused)?));
        entries.push(("evaluation_mode", any(py, result.evaluation_mode.value())?));
        entries.extend([
            ("measured_value", measured),
            ("sample_count", any(py, result.sample_count)?),
            ("sample_unit", any(py, &result.sample_unit)?),
            ("minimum_samples", any(py, result.minimum_samples)?),
        ]);
        entries.push((
            "thresholds",
            measurement_thresholds(&any(py, &result.thresholds)?)?,
        ));
        entries.push((
            "evidence_count",
            any(py, measurement.then_some(result.evidence_rows.len()))?,
        ));
        entries.push((
            "evidence_truncated",
            any(py, measurement.then_some(result.evidence_truncated))?,
        ));
        entries.extend([
            ("execution_error", any(py, &result.execution_error)?),
            ("skip_reason", any(py, &result.skip_reason)?),
        ]);
        checks.push(without_none(py, entries)?);
    }
    Ok(checks)
}

/// `pass`, `warn` or `fail` of a Python check result.
pub(crate) fn python_check_status(result: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    Ok(if native_field(result, "passed")?.is_truthy()? {
        "pass"
    } else if native_field(result, "warned")?.is_truthy()? {
        "warn"
    } else {
        "fail"
    })
}

/// Python check results.
pub(crate) fn python_checks<'py>(
    projection: &Projection<'py>,
    results: &[Bound<'py, PyAny>],
) -> PyResult<Vec<Bound<'py, PyDict>>> {
    let mut checks: Vec<Bound<'py, PyDict>> = Vec::new();
    for result in results {
        if !projection.terminal_recorded(result)? {
            continue;
        }
        let py: Python<'py> = result.py();
        let name: String = text(&native_field(result, "node_name")?)?;
        checks.push(without_none(
            py,
            vec![
                ("kind", any(py, "python_check")?),
                ("name", native_field(result, "node_name")?),
                ("check_id", any(py, format!("python_check:{name}"))?),
                ("passed", native_field(result, "passed")?),
                ("status", any(py, python_check_status(result)?)?),
                ("severity", value(result, "severity")?),
                ("message", native_field(result, "message")?),
                ("error_message", native_field(result, "error_message")?),
                ("metadata", native_field(result, "metadata")?),
                (
                    "warnings",
                    or_none(native_field(result, "warning_messages")?)?,
                ),
            ],
        )?);
    }
    Ok(checks)
}

fn with_errors<'py>(
    result: &Bound<'py, PyAny>,
    mut entries: Vec<(&'static str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyDict>> {
    entries.extend(fields(result, &ERROR_FIELDS)?);
    without_none(result.py(), entries)
}

/// The scenario check with its expected-result and assertion outcomes.
pub(crate) fn scenario_check<'py>(
    py: Python<'py>,
    result: &ScenarioRunResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let name = &result.scenario_name;
    let mut expected: Vec<Bound<'py, PyDict>> = Vec::new();
    for item in &result.expected_results {
        let counts = [
            ("actual_row_count", item.actual_row_count),
            ("expected_row_count", item.expected_row_count),
            ("mismatched_row_count", item.mismatched_row_count),
            ("missing_row_count", item.missing_row_count),
        ];
        let compared = counts.iter().all(|(_, count)| count.is_some());
        let mut entries: Vec<(&'static str, Bound<'py, PyAny>)> = vec![
            ("model_name", any(py, &item.model_name)?),
            ("status", any(py, item.status.value())?),
            (
                "comparison_status",
                any(
                    py,
                    if compared {
                        COMPARISON_COMPLETED
                    } else {
                        COMPARISON_NOT_RUN
                    },
                )?,
            ),
        ];
        for (name, count) in counts {
            entries.push((name, any(py, count)?));
        }
        let result: Bound<'py, PyDict> = dict(py, entries)?;
        for (name, error) in scenario_errors(
            py,
            (&item.error_code, &item.error_help, &item.error_message),
        )? {
            if !error.is_none() {
                result.set_item(name, error)?;
            }
        }
        expected.push(result);
    }
    let mut assertions: Vec<Bound<'py, PyDict>> = Vec::new();
    for item in &result.assertion_results {
        let ran: &str = if item.failing_row_count.is_none() {
            COMPARISON_NOT_RUN
        } else {
            COMPARISON_COMPLETED
        };
        let result: Bound<'py, PyDict> = dict(
            py,
            vec![
                ("name", any(py, &item.name)?),
                ("status", any(py, item.status.value())?),
                ("query_status", any(py, ran)?),
                ("failing_row_count", any(py, item.failing_row_count)?),
                ("sample_rows", any(py, &item.sample_rows)?),
            ],
        )?;
        for (name, error) in scenario_errors(
            py,
            (&item.error_code, &item.error_help, &item.error_message),
        )? {
            if !error.is_none() {
                result.set_item(name, error)?;
            }
        }
        assertions.push(result);
    }
    let mut entries = vec![
        ("kind", any(py, "scenario")?),
        ("name", any(py, name.as_str())?),
        ("check_id", any(py, format!("sql_scenario:{name}"))?),
        (
            "passed",
            any(py, result.status == ExecutionStatus::Success)?,
        ),
        ("status", any(py, result.status.value())?),
        ("expected_results", tuple_of(py, expected)?),
        ("assertion_results", tuple_of(py, assertions)?),
    ];
    entries.extend(scenario_errors(
        py,
        (
            &result.error_code,
            &result.error_help,
            &result.error_message,
        ),
    )?);
    without_none(py, entries)
}

fn scenario_errors<'py>(
    py: Python<'py>,
    (code, help, message): (&Option<String>, &Option<String>, &Option<String>),
) -> PyResult<Vec<(&'static str, Bound<'py, PyAny>)>> {
    Ok(vec![
        ("error_code", any(py, code)?),
        ("error_help", any(py, help)?),
        ("error_message", any(py, message)?),
    ])
}

/// The run outcome of one scenario.
pub(crate) fn scenario_result<'py>(
    py: Python<'py>,
    result: &ScenarioRunResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let duration = match result.duration_ms {
        Some(duration) => rounded(py, duration)?,
        None => py.None().into_bound(py),
    };
    let mut entries = vec![
        ("name", any(py, &result.scenario_name)?),
        ("status", any(py, result.status.value())?),
        ("duration_ms", duration),
        (
            "local_status",
            any(py, result.local_status.map(|status| status.value()))?,
        ),
        ("retained", any(py, result.retained)?),
        (
            "local_duckdb_path",
            any(
                py,
                result
                    .local_duckdb_path
                    .as_ref()
                    .map(|path| path.as_os_str()),
            )?,
        ),
    ];
    entries.extend(scenario_errors(
        py,
        (
            &result.error_code,
            &result.error_help,
            &result.error_message,
        ),
    )?);
    without_none(py, entries)
}

/// The capture outcome of one scenario snapshot sync or refresh.
pub(crate) fn snapshot_result<'py>(result: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = result.py();
    let capture: Bound<'py, PyAny> = native_field(result, "capture_result")?;
    let mut relations: Vec<Bound<'py, PyDict>> = Vec::new();
    let mut manifest_path: Bound<'py, PyAny> = py.None().into_bound(py);
    if !capture.is_none() {
        for relation in items(&native_field(&capture, "relation_results")?)? {
            relations.push(with_errors(
                &relation,
                vec![
                    ("kind", value(&relation, "kind")?),
                    ("logical_name", native_field(&relation, "logical_name")?),
                    (
                        "source_relation",
                        native_field(&relation, "source_relation")?,
                    ),
                    (
                        "file_path",
                        native_field(&relation, "file_path")?.str()?.into_any(),
                    ),
                    ("status", value(&relation, "status")?),
                    ("row_count", native_field(&relation, "row_count")?),
                    ("byte_count", native_field(&relation, "byte_count")?),
                ],
            )?);
        }
        manifest_path = native_field(&capture, "manifest_path")?.str()?.into_any();
    }
    with_errors(
        result,
        vec![
            ("name", native_field(result, "scenario_name")?),
            ("status", value(result, "status")?),
            ("retained", native_field(result, "retained")?),
            ("relations", tuple_of(py, relations)?),
            ("manifest_path", manifest_path),
        ],
    )
}
