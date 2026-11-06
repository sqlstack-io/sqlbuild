//! Integration-result records built from a claimed terminal and its executor result.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFloat, PyInt};
use sqlbuild_core::integration_results::errors::IntegrationRecordError;
use sqlbuild_core::integration_results::main::encode_integration_value::encode_integration_value;
use sqlbuild_core::integration_results::main::normalize_integration_record::normalize_integration_record;
use sqlbuild_core::json::models::{JsonInteger, JsonValue};
use sqlbuild_host::models::{
    AuditEvaluationMode, AuditExecutionResult, AuditOutcome, FunctionExecutionResult,
    LoadExecutionResult, ModelExecutionResult, SeedExecutionResult, SqlTestExecutionResult,
    SqlTestOutcome,
};
use sqlbuild_host::types::ResultEnum;

use crate::bindings::_helpers::native_host::results::ReadResult;
use crate::bindings::_helpers::plan_output::safety::{future_cursor_safety, maximum_start_safety};
use crate::bindings::_helpers::plan_output::values::any;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::result_output::assets::microbatch_summary;
use crate::bindings::_helpers::result_output::checks::{
    measured_value, measurement_thresholds, python_check_status,
};
use crate::bindings::_helpers::result_output::integration_values::{
    Checked, bounded_metadata, optional_text, required_json, text, text_or_empty,
};
use crate::bindings::_helpers::result_output::projection::{
    ResultIdentity, ResultKind, result_kind,
};

const FAILED_EVENT: &str = "resource_attempt_failed";
const SKIPPED_EVENT: &str = "resource_attempt_skipped";

type Entries = Vec<(String, JsonValue)>;

/// Entries from Python values, failing the contract on a non-JSON value.
fn entries(values: Vec<(&str, Bound<'_, PyAny>)>) -> Checked<Entries> {
    let mut converted: Entries = Vec::with_capacity(values.len());
    for (key, value) in values {
        match required_json(&value)? {
            Ok(value) => converted.push((key.to_owned(), value)),
            Err(error) => return Ok(Err(error)),
        }
    }
    Ok(Ok(converted))
}

fn value<'py>(result: &Bound<'py, PyAny>, field: &str) -> PyResult<Bound<'py, PyAny>> {
    native_field(&native_field(result, field)?, "value")
}

/// `plan.<field>[name].qualified_name` when the plan locates the resource.
fn planned_target<'py>(
    plan: &Bound<'py, PyAny>,
    field: &str,
    name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan.py();
    if plan.is_none() {
        return Ok(py.None().into_bound(py));
    }
    let locations: Bound<'py, PyAny> = native_field(plan, field)?;
    if !locations.contains(name)? {
        return Ok(py.None().into_bound(py));
    }
    native_field(&locations.get_item(name)?, "qualified_name")
}

fn model_microbatch<'py>(
    py: Python<'py>,
    result: &ModelExecutionResult<Bound<'py, PyAny>>,
) -> PyResult<JsonValue> {
    let rendered: Bound<'py, PyAny> = microbatch_summary(py, result)?;
    if rendered.is_none() {
        return Ok(JsonValue::Object(Vec::new()));
    }
    let values: Bound<'py, PyDict> = PyDict::new(py);
    for (key, item) in rendered.downcast::<PyDict>()?.iter() {
        let key: String = key.extract()?;
        if !item.is_none() {
            values.set_item(key, item)?;
        }
    }
    if let Some(limit) = result.microbatch_limit {
        values.set_item("limit", limit)?;
    }
    if let Some(count) = result.microbatch_limit_count {
        values.set_item("count", count)?;
    }
    if let Some(action) = result.microbatch_limit_action {
        values.set_item("action", action.value())?;
    }
    bounded_metadata(values.as_any())
}

/// `_asset_result`: the asset enrichment of an asset-producing result.
fn asset<'py>(result: &Bound<'py, PyAny>, plan: &Bound<'py, PyAny>) -> Checked<Option<Entries>> {
    let py: Python<'py> = result.py();
    let none = || py.None().into_bound(py);
    let values: Vec<(&str, Bound<'py, PyAny>)> = match result_kind(result)? {
        ResultKind::Model => {
            let native: ModelExecutionResult<Bound<'py, PyAny>> =
                ModelExecutionResult::read(result)?;
            let name: Bound<'py, PyAny> = any(py, &native.model_name)?;
            let planned: Bound<'py, PyAny> = planned_target(plan, "model_locations", &name)?;
            let mut values: Vec<(&str, Bound<'py, PyAny>)> = vec![
                ("kind", "model".into_pyobject(py)?.into_any()),
                ("name", name),
                ("status", any(py, native.status.value())?),
                (
                    "target",
                    if planned.is_truthy()? {
                        planned
                    } else {
                        any(py, &native.promoted_relation)?
                    },
                ),
                ("staging_relation", any(py, &native.staging_relation)?),
                (
                    "failed_phase",
                    any(py, native.failed_phase.map(ResultEnum::value))?,
                ),
            ];
            let mut converted: Entries = match entries(std::mem::take(&mut values))? {
                Ok(converted) => converted,
                Err(error) => return Ok(Err(error)),
            };
            for (key, evidence) in [
                (
                    "future_cursor_safety",
                    future_cursor_safety(&any(py, &native.future_cursor_safety)?)?,
                ),
                (
                    "maximum_start_safety",
                    maximum_start_safety(&any(py, &native.maximum_start_safety)?)?,
                ),
            ] {
                let evidence: Bound<'py, PyAny> = if evidence.is_none() {
                    pyo3::types::PyDict::new(py).into_any()
                } else {
                    evidence
                };
                converted.push((key.to_owned(), bounded_metadata(&evidence)?));
            }
            converted.push(("microbatch".to_owned(), model_microbatch(py, &native)?));
            return Ok(Ok(Some(converted)));
        }
        ResultKind::Seed => {
            let native = SeedExecutionResult::read(result)?;
            let name: Bound<'py, PyAny> = any(py, &native.seed_name)?;
            vec![
                ("kind", "seed".into_pyobject(py)?.into_any()),
                ("target", planned_target(plan, "seed_locations", &name)?),
                ("name", name),
                ("status", any(py, native.status.value())?),
            ]
        }
        ResultKind::Function => {
            let native = FunctionExecutionResult::read(result)?;
            let name: Bound<'py, PyAny> = any(py, &native.function_name)?;
            vec![
                ("kind", any(py, &native.function_kind)?),
                ("target", planned_target(plan, "function_locations", &name)?),
                ("name", name),
                ("status", any(py, native.status.value())?),
            ]
        }
        ResultKind::Load => {
            let native: LoadExecutionResult<Bound<'py, PyAny>> = LoadExecutionResult::read(result)?;
            vec![
                ("kind", any(py, native.resource_kind.value())?),
                ("name", any(py, &native.source_name)?),
                ("status", any(py, native.status.value())?),
                ("target", any(py, &native.target)?),
                ("staging_relation", any(py, &native.staging_relation)?),
                ("loader", any(py, &native.loader_name)?),
                ("rows_loaded", any(py, native.rows_loaded)?),
            ]
        }
        ResultKind::PythonNode => vec![
            ("kind", value(result, "kind")?),
            ("name", native_field(result, "node_name")?),
            ("status", value(result, "status")?),
            ("materialized", native_field(result, "materialized")?),
        ],
        _ => {
            let _ = none;
            return Ok(Ok(None));
        }
    };
    Ok(entries(values)?.map(Some))
}

fn audit_check<'py>(result: &Bound<'py, PyAny>) -> Checked<Entries> {
    let py: Python<'py> = result.py();
    let native: AuditExecutionResult<Bound<'py, PyAny>> = AuditExecutionResult::read(result)?;
    let check_id = any(py, native.identity()?.map(|(_, id)| id))?;
    let measurement = native.evaluation_mode == AuditEvaluationMode::Measurement;
    entries(vec![
        ("kind", "audit".into_pyobject(py)?.into_any()),
        ("name", any(py, &native.audit_name)?),
        ("check_id", check_id.clone()),
        ("dag_check_id", check_id),
        (
            "passed",
            any(
                py,
                matches!(
                    native.outcome,
                    AuditOutcome::Pass | AuditOutcome::Insufficient
                ),
            )?,
        ),
        ("status", any(py, native.outcome.value())?),
        ("severity", any(py, native.severity.value())?),
        ("row_count", any(py, native.row_count)?),
        ("attachment_kind", any(py, native.attachment_kind.value())?),
        ("asset_name", any(py, &native.attached_target_name)?),
        (
            "attached_column_name",
            any(py, &native.attached_column_name)?,
        ),
        (
            "attached_target_name",
            any(py, &native.attached_target_name)?,
        ),
        ("run_scope_phase", any(py, native.run_scope_phase.value())?),
        ("reused", any(py, native.reused)?),
        ("evaluation_mode", any(py, native.evaluation_mode.value())?),
        (
            "measured_value",
            measured_value(py, native.measured_value.as_ref())?,
        ),
        ("sample_count", any(py, native.sample_count)?),
        ("sample_unit", any(py, &native.sample_unit)?),
        ("minimum_samples", any(py, native.minimum_samples)?),
        (
            "thresholds",
            measurement_thresholds(&any(py, &native.thresholds)?)?,
        ),
        (
            "evidence_count",
            any(py, measurement.then_some(native.evidence_rows.len()))?,
        ),
        (
            "evidence_truncated",
            any(py, measurement.then_some(native.evidence_truncated))?,
        ),
        (
            "audit_definition_name",
            any(py, &native.audit_definition_name)?,
        ),
        ("audit_description", any(py, &native.audit_description)?),
    ])
}

/// `_check_results`: the check enrichment of a check-producing result.
fn checks<'py>(result: &Bound<'py, PyAny>) -> Checked<Vec<Entries>> {
    let py: Python<'py> = result.py();
    let single = |checked: Checked<Entries>| -> Checked<Vec<Entries>> {
        Ok(checked?.map(|check| vec![check]))
    };
    match result_kind(result)? {
        ResultKind::Audit => single(audit_check(result)),
        ResultKind::SqlTest => {
            let native: SqlTestExecutionResult<Bound<'py, PyAny>> =
                SqlTestExecutionResult::read(result)?;
            let check_id = any(py, native.identity()?.map(|(_, id)| id))?;
            single(entries(vec![
                ("kind", "sql_test".into_pyobject(py)?.into_any()),
                ("name", any(py, &native.test_name)?),
                ("check_id", check_id.clone()),
                ("dag_check_id", check_id),
                ("passed", any(py, native.outcome == SqlTestOutcome::Pass)?),
                ("status", any(py, native.outcome.value())?),
                (
                    "asset_name",
                    match native.step_results.first() {
                        Some(step) => any(py, &step.model_name)?,
                        None => py.None().into_bound(py),
                    },
                ),
            ]))
        }
        ResultKind::PythonCheck => {
            let name: String = native_field(result, "node_name")?.extract()?;
            let check_id: Bound<'py, PyAny> = format!("check:{name}").into_pyobject(py)?.into_any();
            single(entries(vec![
                ("kind", "python_check".into_pyobject(py)?.into_any()),
                ("name", native_field(result, "node_name")?),
                ("check_id", check_id.clone()),
                ("dag_check_id", check_id),
                ("passed", native_field(result, "passed")?),
                (
                    "status",
                    python_check_status(result)?.into_pyobject(py)?.into_any(),
                ),
                ("severity", value(result, "severity")?),
            ]))
        }
        _ => Ok(Ok(Vec::new())),
    }
}

/// The payload attempt number when it is an `int`, else 0.
fn attempt_number(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    if value.is_instance_of::<PyInt>() && !value.is_instance_of::<PyBool>() {
        let decimal: String = value.str()?.to_string();
        return Ok(JsonInteger::parse(&decimal).map_or(JsonValue::Null, JsonValue::Integer));
    }
    Ok(JsonValue::Integer(JsonInteger::from(0_i64)))
}

/// The payload duration when it is a number, `bool` included, else `null`.
fn duration(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    if value.is_instance_of::<PyInt>() || value.is_instance_of::<PyFloat>() {
        return Ok(required_json(value)?.unwrap_or(JsonValue::Null));
    }
    Ok(JsonValue::Null)
}

/// The envelope fields every record copies from its terminal, in record order.
fn terminal_fields(terminal: &Bound<'_, PyAny>, event_sequence: usize) -> PyResult<Entries> {
    let payload: Bound<'_, PyAny> = native_field(terminal, "payload")?;
    let get = |key: &str| mapping_get(&payload, key);
    let attribute = |key: &str| -> PyResult<JsonValue> {
        Ok(required_json(&native_field(terminal, key)?)?.unwrap_or(JsonValue::Null))
    };
    let mut fields: Entries = vec![
        (
            "schema_version".to_owned(),
            JsonValue::Integer(JsonInteger::from(1_i64)),
        ),
        ("record_kind".to_owned(), text("integration_result")),
        ("event_id".to_owned(), attribute("event_id")?),
        (
            "event_sequence".to_owned(),
            JsonValue::Integer(JsonInteger::from(event_sequence as u64)),
        ),
        ("event_type".to_owned(), attribute("event_type")?),
        (
            "occurred_at".to_owned(),
            optional_text(&native_field(terminal, "occurred_at")?.call_method0("isoformat")?)?,
        ),
        ("invocation_id".to_owned(), attribute("invocation_id")?),
    ];
    for key in ["run_id", "resource_id", "resource_attempt_id"] {
        fields.push((
            key.to_owned(),
            text_or_empty(&native_field(terminal, key)?)?,
        ));
    }
    fields.push(("operation_id".to_owned(), attribute("operation_id")?));
    fields.push(("statement_id".to_owned(), attribute("statement_id")?));
    fields.push((
        "resource_kind".to_owned(),
        text_or_empty(&get("resource_kind")?)?,
    ));
    fields.push((
        "resource_name".to_owned(),
        text_or_empty(&get("resource_name")?)?,
    ));
    fields.push((
        "attempt_number".to_owned(),
        attempt_number(&get("attempt_number")?)?,
    ));
    fields.push(("duration_ms".to_owned(), duration(&get("duration_ms")?)?));
    Ok(fields)
}

fn failure_fields(terminal: &Bound<'_, PyAny>) -> PyResult<Entries> {
    let payload: Bound<'_, PyAny> = native_field(terminal, "payload")?;
    let mut fields: Entries = Vec::new();
    for key in ["error_code", "error_type", "skip_code", "skip_mode"] {
        fields.push((key.to_owned(), optional_text(&mapping_get(&payload, key)?)?));
    }
    Ok(fields)
}

/// `_canonical_status`: failed or skipped from the terminal, else the result's own status.
fn canonical_status(event_type: &str, fallback: JsonValue) -> JsonValue {
    match event_type {
        FAILED_EVENT => text("failed"),
        SKIPPED_EVENT => text("skipped"),
        _ => fallback,
    }
}

fn payload_text(terminal: &Bound<'_, PyAny>, key: &str) -> PyResult<JsonValue> {
    let value: JsonValue =
        optional_text(&native_field(terminal, "payload")?.call_method1("get", (key,))?)?;
    Ok(match value {
        JsonValue::Null => text(""),
        other => other,
    })
}

/// The complete record, encoded as one JSON Lines record with its newline.
fn encoded(
    terminal: &Bound<'_, PyAny>,
    event_sequence: usize,
    (command, degraded): (&str, bool),
    (asset, checks): (Option<Entries>, Vec<Entries>),
) -> Checked<String> {
    let mut record: Entries = terminal_fields(terminal, event_sequence)?;
    let output_kind: &str = if asset.is_some() { "asset" } else { "check" };
    record.push(("output_kind".to_owned(), text(output_kind)));
    record.push(("command".to_owned(), text(command)));
    record.extend(failure_fields(terminal)?);
    record.push(("projection_degraded".to_owned(), JsonValue::Bool(degraded)));
    record.push((
        "asset".to_owned(),
        asset.map_or(JsonValue::Null, JsonValue::Object),
    ));
    record.push((
        "checks".to_owned(),
        JsonValue::Array(checks.into_iter().map(JsonValue::Object).collect()),
    ));
    let normalized: Entries = match normalize_integration_record(JsonValue::Object(record)) {
        Ok(normalized) => normalized,
        Err(error) => return Ok(Err(error)),
    };
    Ok(encode_integration_value(&JsonValue::Object(normalized), true).map(|line| line + "\n"))
}

fn event_type(terminal: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(terminal, "event_type")?.extract()
}

/// `build_integration_result`, encoded; `None` when the result carries no enrichment.
pub(crate) fn integration_record(
    (result, plan): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (terminal, event_sequence): (&Bound<'_, PyAny>, usize),
    command: &str,
) -> Checked<Option<String>> {
    let mut asset: Option<Entries> = match asset(result, plan)? {
        Ok(asset) => asset,
        Err(error) => return Ok(Err(error)),
    };
    if let Some(entries) = &mut asset {
        let event: String = event_type(terminal)?;
        let kind: JsonValue = payload_text(terminal, "resource_kind")?;
        let name: JsonValue = payload_text(terminal, "resource_name")?;
        for (key, value) in entries.iter_mut() {
            match key.as_str() {
                "kind" => *value = kind.clone(),
                "name" => *value = name.clone(),
                "status" => {
                    *value = canonical_status(&event, std::mem::replace(value, JsonValue::Null))
                }
                _ => {}
            }
        }
    }
    let checks: Vec<Entries> = match checks(result)? {
        Ok(checks) => checks,
        Err(error) => return Ok(Err(error)),
    };
    if asset.is_none() && checks.is_empty() {
        return Ok(Ok(None));
    }
    Ok(encoded(terminal, event_sequence, (command, false), (asset, checks))?.map(Some))
}

/// `build_clone_integration_result`, encoded.
pub(crate) fn clone_record(
    item: &Bound<'_, PyAny>,
    (terminal, event_sequence): (&Bound<'_, PyAny>, usize),
) -> Checked<String> {
    let event: String = event_type(terminal)?;
    let values: Entries = match entries(vec![
        ("action", value(item, "action")?),
        ("target", native_field(item, "destination_relation")?),
        ("origin_relation", native_field(item, "origin_relation")?),
    ])? {
        Ok(values) => values,
        Err(error) => return Ok(Err(error)),
    };
    let mut asset: Entries = vec![
        (
            "kind".to_owned(),
            payload_text_or_str(terminal, "resource_kind")?,
        ),
        (
            "name".to_owned(),
            payload_text_or_str(terminal, "resource_name")?,
        ),
        (
            "status".to_owned(),
            canonical_status(
                &event,
                required_json(&value(item, "status")?)?.unwrap_or(JsonValue::Null),
            ),
        ),
    ];
    asset.extend(values);
    encoded(
        terminal,
        event_sequence,
        ("clone", false),
        (Some(asset), Vec::new()),
    )
}

/// `str(payload.get(key) or "")`, as the clone asset's identity.
fn payload_text_or_str(terminal: &Bound<'_, PyAny>, key: &str) -> PyResult<JsonValue> {
    text_or_empty(&native_field(terminal, "payload")?.call_method1("get", (key,))?)
}

/// `build_degraded_integration_result`, encoded; `None` for results without enrichment.
pub(crate) fn degraded_record(
    result: &Bound<'_, PyAny>,
    (terminal, event_sequence): (&Bound<'_, PyAny>, usize),
    command: &str,
) -> Checked<Option<String>> {
    let event: String = event_type(terminal)?;
    let kind: JsonValue = payload_text_or_str(terminal, "resource_kind")?;
    let name: JsonValue = payload_text_or_str(terminal, "resource_name")?;
    let (asset, checks): (Option<Entries>, Vec<Entries>) = match result_kind(result)? {
        ResultKind::Model
        | ResultKind::Seed
        | ResultKind::Function
        | ResultKind::Load
        | ResultKind::PythonNode
        | ResultKind::CloneItem => {
            let status: &str = match event.as_str() {
                FAILED_EVENT => "failed",
                SKIPPED_EVENT => "skipped",
                _ => "success",
            };
            (
                Some(vec![
                    ("kind".to_owned(), kind),
                    ("name".to_owned(), name),
                    ("status".to_owned(), text(status)),
                ]),
                Vec::new(),
            )
        }
        ResultKind::Audit | ResultKind::SqlTest | ResultKind::PythonCheck => {
            let failed: bool = event == FAILED_EVENT;
            let check_id: JsonValue = text_or_empty(&native_field(terminal, "resource_id")?)?;
            (
                None,
                vec![vec![
                    ("kind".to_owned(), kind),
                    ("name".to_owned(), name),
                    ("check_id".to_owned(), check_id),
                    ("passed".to_owned(), JsonValue::Bool(!failed)),
                    (
                        "status".to_owned(),
                        text(if failed { "error" } else { "pass" }),
                    ),
                ]],
            )
        }
        ResultKind::Other => return Ok(Ok(None)),
    };
    Ok(encoded(terminal, event_sequence, (command, true), (asset, checks))?.map(Some))
}

/// Whether the record failure should not hide a completed warehouse execution.
pub(crate) fn execution_succeeded(terminal: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(event_type(terminal)?.ends_with("completed"))
}

/// The warning written before a degraded record.
pub(crate) fn degraded_warning(error: &IntegrationRecordError, succeeded: bool) -> String {
    let completion: &str = if succeeded {
        "warehouse execution completed successfully"
    } else {
        "execution result remains authoritative"
    };
    format!(
        "warning: integration output degraded: {}; {completion}\n",
        error.message
    )
}
