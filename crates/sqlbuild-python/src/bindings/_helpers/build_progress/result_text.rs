//! Executor result objects read into the plain data build progress output renders.

use std::collections::HashMap;

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_core::build_progress::models::{
    AuditResultText, BuildResultText, ErrorText, HookResultText, LifecycleEventText, LoadRowText,
    ModelResultText, NodeResultText, PythonNodeText, SkipText, TestResultText, TestStepText,
};
use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::main::python_float_repr::python_float_repr;
use sqlbuild_core::json::models::{JsonDialect, StdlibJsonOptions};
use sqlbuild_core::progress::main::format_expectation_detail::format_expectation_detail;
use sqlbuild_core::progress::models::ExpectationStep;
use sqlbuild_core::test_command::main::parameterized_test_label::parameterized_test_label;
use sqlbuild_core::test_command::models::TestParameter;
use sqlbuild_host::models::{
    AuditEvaluationMode, AuditExecutionResult, BuildExecutionResult, BuildStatus, ExecutionStatus,
    FunctionExecutionResult, HookExecutionResult, HookPhase, LifeCycleEvent, LifeCycleEventKind,
    LoadExecutionResult, MeasuredValue, ModelExecutionResult, SeedExecutionResult,
    SqlTestExecutionResult, StepResult,
};
use sqlbuild_host::types::ResultEnum;

use crate::bindings::_helpers::native_host::results::ReadResult;
use crate::bindings::_helpers::plan_output::display::{cursor_bound_display, inclusive_cursor_end};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::result_output::integration_values::json_value;

const DECIMAL_KIND: &str = "decimal";

pub(crate) fn text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    Ok(native_field(value, name)?.str()?.to_string())
}

pub(crate) fn optional(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    let field: Bound<'_, PyAny> = native_field(value, name)?;
    if field.is_none() {
        return Ok(None);
    }
    Ok(Some(field.str()?.to_string()))
}

fn texts(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<String>> {
    native_field(value, name)?
        .try_iter()?
        .map(|item| Ok(item?.str()?.to_string()))
        .collect()
}

fn error(code: &Option<String>, message: &Option<String>, help: &Option<String>) -> ErrorText {
    ErrorText {
        code: code.clone(),
        message: message.clone(),
        help: help.clone(),
    }
}

fn events(value: &[LifeCycleEvent]) -> Vec<LifecycleEventText> {
    value
        .iter()
        .map(|event| match event.kind {
            LifeCycleEventKind::Sql => LifecycleEventText::Sql(event.content.clone()),
            LifeCycleEventKind::Log => LifecycleEventText::Log(event.content.clone()),
        })
        .collect()
}

pub(crate) fn audit_text(audit: &Bound<'_, PyAny>) -> PyResult<AuditResultText> {
    Ok(audit_result_text(&AuditExecutionResult::read(audit)?))
}

fn audit_result_text(audit: &AuditExecutionResult<Bound<'_, PyAny>>) -> AuditResultText {
    AuditResultText {
        audit_name: audit.audit_name.clone(),
        attached_column_name: audit.attached_column_name.clone(),
        attached_target_name: audit.attached_target_name.clone(),
        run_scope_phase: audit.run_scope_phase.value().to_owned(),
        outcome: audit.outcome.value().to_owned(),
        row_count: audit.row_count,
        reused: audit.reused,
        executed_sql: Some(audit.executed_sql.clone()),
        measurement: audit.evaluation_mode == AuditEvaluationMode::Measurement,
        measured_value: match &audit.measured_value {
            Some(MeasuredValue::Integer(value)) => value.as_str().to_owned(),
            Some(MeasuredValue::Float(value)) => python_float_repr(*value),
            None => "None".to_owned(),
        },
        execution_error: audit.execution_error.clone(),
        skip_reason: audit.skip_reason.clone(),
    }
}

fn audits(value: &[AuditExecutionResult<Bound<'_, PyAny>>]) -> Vec<AuditResultText> {
    value.iter().map(audit_result_text).collect()
}

fn hook_text(hook: &HookExecutionResult) -> HookResultText {
    HookResultText {
        pre_hook: hook.phase == HookPhase::PreHooks,
        hook_type: hook.hook_type.clone(),
        label: hook.label.clone(),
        status: hook.status.value().to_owned(),
        skip: SkipText {
            mode: hook.skip_mode.map(|mode| mode.value().to_owned()),
            reason: hook.skip_reason.clone(),
        },
    }
}

fn cursor_range(
    py: Python<'_>,
    model: &ModelExecutionResult<Bound<'_, PyAny>>,
) -> PyResult<Option<(String, String)>> {
    let (Some(start), Some(end)) = (&model.cursor_range_start, &model.cursor_range_end) else {
        return Ok(None);
    };
    if !model.empty_cursor_inputs.is_empty() {
        return Ok(None);
    }
    let start = start.into_bound_py_any(py)?;
    let end = end.into_bound_py_any(py)?;
    let cursor_type = model.cursor_type.as_deref().into_bound_py_any(py)?;
    let cursor_grain = model.cursor_grain.as_deref().into_bound_py_any(py)?;
    Ok(Some((
        cursor_bound_display(&start, &cursor_type, &cursor_grain)?,
        inclusive_cursor_end(&end, &cursor_type, &cursor_grain)?,
    )))
}

pub(crate) fn model_text(model: &Bound<'_, PyAny>) -> PyResult<ModelResultText> {
    model_result_text(model.py(), &ModelExecutionResult::read(model)?)
}

fn model_result_text(
    py: Python<'_>,
    model: &ModelExecutionResult<Bound<'_, PyAny>>,
) -> PyResult<ModelResultText> {
    Ok(ModelResultText {
        name: model.model_name.clone(),
        status: model.status.value().to_owned(),
        failed_phase: model.failed_phase.map(|phase| phase.value().to_owned()),
        staging_relation: model.staging_relation.clone(),
        duration_ms: model.duration_ms.map(|duration| duration as f64),
        batch_count: model.batch_count,
        batch_size: model.batch_size.clone(),
        rows_affected: model.rows_affected,
        cursor_range: cursor_range(py, model)?,
        empty_cursor_inputs: model.empty_cursor_inputs.clone(),
        waiting_on_empty_inputs: model.waiting_on_empty_inputs,
        recovery_batch_count: model.microbatch_recovery_batch_count,
        synthetic_completion_count: model.microbatch_synthetic_completion_count,
        replay_requirement_state: model.microbatch_replay_requirement_state.clone(),
        audits: audits(&model.audit_results),
        warnings: model.warning_messages.clone(),
        events: events(&model.lifecycle_events),
        hooks: model.hook_results.iter().map(hook_text).collect(),
        skip: SkipText {
            mode: model.skip_mode.map(|mode| mode.value().to_owned()),
            reason: model.skip_reason.clone(),
        },
        error: error(&model.error_code, &model.error_message, &model.error_help),
    })
}

pub(crate) fn seed_text(seed: &Bound<'_, PyAny>) -> PyResult<NodeResultText> {
    Ok(seed_result_text(&SeedExecutionResult::read(seed)?))
}

fn seed_result_text(seed: &SeedExecutionResult) -> NodeResultText {
    node_result_text(
        (&seed.seed_name, "seed"),
        (seed.status, seed.duration_ms),
        error(&seed.error_code, &seed.error_message, &seed.error_help),
        &[],
    )
}

pub(crate) fn function_text(function: &Bound<'_, PyAny>) -> PyResult<NodeResultText> {
    Ok(function_result_text(&FunctionExecutionResult::read(
        function,
    )?))
}

fn function_result_text(function: &FunctionExecutionResult) -> NodeResultText {
    node_result_text(
        (&function.function_name, &function.function_kind),
        (function.status, function.duration_ms),
        error(
            &function.error_code,
            &function.error_message,
            &function.error_help,
        ),
        &function.warning_messages,
    )
}

fn node_result_text(
    (name, kind): (&str, &str),
    (status, duration_ms): (ExecutionStatus, Option<i64>),
    error: ErrorText,
    warnings: &[String],
) -> NodeResultText {
    NodeResultText {
        name: name.to_owned(),
        kind: kind.to_owned(),
        status: status.value().to_owned(),
        duration_ms: duration_ms.map(|duration| duration as f64),
        error,
        warnings: warnings.to_vec(),
    }
}

pub(crate) fn load_text(load: &Bound<'_, PyAny>) -> PyResult<LoadRowText> {
    Ok(load_result_text(&LoadExecutionResult::read(load)?))
}

fn load_result_text(load: &LoadExecutionResult<Bound<'_, PyAny>>) -> LoadRowText {
    LoadRowText {
        source_name: load.source_name.clone(),
        loader_name: load.loader_name.clone(),
        resource_kind: load.resource_kind.value().to_owned(),
        status: load.status.value().to_owned(),
        rows_loaded: load.rows_loaded,
        duration_ms: load.duration_ms.map(|duration| duration as f64),
        events: events(&load.lifecycle_events),
        skip: SkipText {
            mode: load.skip_mode.map(|mode| mode.value().to_owned()),
            reason: load.skip_reason.clone(),
        },
        error_message: load.error_message.clone(),
        warnings: load.warning_messages.clone(),
    }
}

pub(crate) fn test_text(test: &Bound<'_, PyAny>) -> PyResult<TestResultText> {
    test_result_text(&SqlTestExecutionResult::read(test)?)
}

fn test_step_text(step: &StepResult) -> TestStepText {
    TestStepText {
        model_name: step.model_name.clone(),
        outcome: step.outcome.value().to_owned(),
        detail: format_expectation_detail(&ExpectationStep {
            model_name: step.model_name.clone(),
            outcome: step.outcome.value().to_owned(),
            actual_row_count: step.actual_row_count,
            expected_row_count: step.expected_row_count,
            mismatched_row_count: step.mismatched_row_count,
            unexpected_row_count: step.unexpected_row_count,
            missing_row_count: step.missing_row_count,
            unexpected_samples: step
                .unexpected_samples
                .iter()
                .map(|sample| sample.values.clone())
                .collect(),
            missing_samples: step
                .missing_samples
                .iter()
                .map(|sample| sample.values.clone())
                .collect(),
            column_differences: step
                .column_differences
                .iter()
                .map(|column| {
                    (
                        column.name.clone(),
                        column.actual.clone(),
                        column.expected.clone(),
                    )
                })
                .collect(),
        }),
    }
}

fn test_result_text(test: &SqlTestExecutionResult<Bound<'_, PyAny>>) -> PyResult<TestResultText> {
    Ok(TestResultText {
        label: test_result_label(test)?,
        outcome: test.outcome.value().to_owned(),
        steps: test.step_results.iter().map(test_step_text).collect(),
        error: error(&test.error_code, &test.error_message, &test.error_help),
    })
}

/// The shared progress/footer/command label of an already-converted executor test result.
pub(crate) fn test_result_label(
    test: &SqlTestExecutionResult<Bound<'_, PyAny>>,
) -> PyResult<String> {
    test_label_values(
        &test.test_name,
        test.source_path
            .as_ref()
            .map(|path| {
                path.to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/")
            })
            .as_deref(),
        &test.parameter_schema,
        &test.parameter_values,
    )
}

pub(crate) fn python_node_text(node: &Bound<'_, PyAny>) -> PyResult<PythonNodeText> {
    Ok(PythonNodeText {
        name: text(node, "node_name")?,
        kind: text(node, "kind")?,
        status: text(node, "status")?,
        error_message: optional(node, "error_message")?,
        warnings: texts(node, "warning_messages")?,
    })
}

pub(crate) fn build_text(result: &Bound<'_, PyAny>) -> PyResult<BuildResultText> {
    let py = result.py();
    let native = BuildExecutionResult::read(result)?;
    build_result_text(py, &native)
}

pub(crate) fn build_result_text(
    py: Python<'_>,
    native: &BuildExecutionResult<Bound<'_, PyAny>>,
) -> PyResult<BuildResultText> {
    Ok(BuildResultText {
        failed: native.status == BuildStatus::Failed,
        warning_count: native.warning_count,
        insufficient_count: native.insufficient_count,
        models: native
            .model_results
            .iter()
            .map(|model| model_result_text(py, model))
            .collect::<PyResult<_>>()?,
        seeds: native.seed_results.iter().map(seed_result_text).collect(),
        functions: native
            .function_results
            .iter()
            .map(function_result_text)
            .collect(),
        loads: native.load_results.iter().map(load_result_text).collect(),
        tests: native
            .test_results
            .iter()
            .map(test_result_text)
            .collect::<PyResult<_>>()?,
        source_audits: audits(&native.source_audit_results),
        end_audits: audits(&native.end_audit_results),
    })
}

/// The label of one test or result: its name, plus source and values for parameterized cases.
pub(crate) fn test_label(item: &Bound<'_, PyAny>, name_field: &str) -> PyResult<String> {
    let name: String = native_field(item, name_field)?.extract()?;
    let source_path: Bound<'_, PyAny> = item.getattr("source_path")?;
    let values = Vec::<(String, Bound<'_, PyAny>)>::read(&native_field(item, "parameter_values")?)?;
    if source_path.is_none() || values.is_empty() {
        return Ok(name);
    }
    let path: String = source_path.call_method0("as_posix")?.extract()?;
    let declarations = Vec::<Bound<'_, PyAny>>::read(&item.getattr("parameter_schema")?)?;
    test_label_values(&name, Some(&path), &declarations, &values)
}

fn test_label_values(
    name: &str,
    path: Option<&str>,
    declarations: &[Bound<'_, PyAny>],
    values: &[(String, Bound<'_, PyAny>)],
) -> PyResult<String> {
    if path.is_none() || values.is_empty() {
        return Ok(name.to_owned());
    }
    let mut schema: HashMap<String, Bound<'_, PyAny>> = HashMap::new();
    for declaration in declarations {
        schema.insert(declaration.getattr("name")?.extract()?, declaration.clone());
    }
    let mut parameters: Vec<TestParameter> = Vec::new();
    for (value_name, value) in values {
        let declaration: &Bound<'_, PyAny> = schema
            .get(value_name)
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(value_name.clone()))?;
        let raw: Bound<'_, PyAny> = value.getattr("value")?;
        let kind: String = value.getattr("kind")?.getattr("value")?.extract()?;
        let payload: Bound<'_, PyAny> = if kind == DECIMAL_KIND {
            raw.str()?.into_any()
        } else {
            raw
        };
        parameters.push(TestParameter {
            name: value_name.clone(),
            value_type: native_field(&native_field(declaration, "value_type")?, "value")?
                .extract()?,
            nullable: declaration.getattr("nullable")?.is_truthy()?,
            rendered: compact_json(&payload)?,
        });
    }
    Ok(parameterized_test_label(name, path, &parameters))
}

/// A payload as `json.dumps(payload, ensure_ascii=True, separators=(",", ":"))`.
fn compact_json(payload: &Bound<'_, PyAny>) -> PyResult<String> {
    let Some(value) = json_value(payload)? else {
        let options: Bound<'_, PyDict> = PyDict::new(payload.py());
        options.set_item("ensure_ascii", true)?;
        options.set_item("separators", (",", ":"))?;
        return payload
            .py()
            .import("json")?
            .call_method("dumps", (payload,), Some(&options))?
            .extract();
    };
    dumps(
        &value,
        &JsonDialect::Stdlib(StdlibJsonOptions::new(None).with_separators(",", ":")),
    )
    .map_err(|error| PyValueError::new_err(format!("{error:?}")))
}
