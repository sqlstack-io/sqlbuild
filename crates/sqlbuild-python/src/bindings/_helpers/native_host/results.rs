//! Read executor dataclasses once into the host contract. Only opaque payloads retain Python values.

use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyInt;
use sqlbuild_core::json::models::JsonInteger;
use sqlbuild_core::scenario_command::models::RetainedRelation;
use sqlbuild_host::models::{
    AttachedAuditTargetKind, AuditAttachmentKind, AuditEvaluationMode, AuditExecutionResult,
    AuditOutcome, AuditRunScope, AuditSeverity, BuildExecutionResult, BuildExecutionTimings,
    BuildStatus, CancellationOutcome, ExecutionPhase, ExecutionResourceKind, ExecutionStatus,
    FunctionExecutionResult, HookExecutionResult, HookPhase, ItemKind, ItemTimeoutReport,
    LifeCycleEvent, LifeCycleEventKind, LoadExecutionResult, MeasuredValue,
    MicrobatchAccountingInterval, MicrobatchLimitAction, ModelExecutionResult,
    ScenarioAssertionExpectationExecutionResult, ScenarioExpectedExpectationExecutionResult,
    ScenarioLocalRunStatus, ScenarioRunResult, SeedExecutionResult, SkipMode,
    SqlTestColumnDifference, SqlTestDifferenceSample, SqlTestExecutionResult, SqlTestOutcome,
    StepResult,
};
use sqlbuild_host::types::ResultEnum;

use crate::bindings::_helpers::records::construct::enum_from_value;
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::records::ids::RecordId;

/// Extraction at the permanent Python executor edge, including nested result records.
pub(crate) trait ReadResult<'py>: Sized {
    fn read(value: &Bound<'py, PyAny>) -> PyResult<Self>;
}

macro_rules! scalar {
    ($($kind:ty),+ $(,)?) => {$ (
        impl<'py> ReadResult<'py> for $kind {
            fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
                value.extract()
            }
        }
    )+};
}

scalar!(String, i64, f64, bool, PathBuf);

impl<'py> ReadResult<'py> for MeasuredValue {
    fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        if value.is_instance_of::<PyInt>() {
            let decimal = value.str()?.to_string();
            return JsonInteger::parse(&decimal)
                .map(Self::Integer)
                .ok_or_else(|| PyValueError::new_err(format!("invalid integer text {decimal}")));
        }
        value.extract().map(Self::Float)
    }
}

impl<'py> ReadResult<'py> for Bound<'py, PyAny> {
    fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(value.clone())
    }
}

impl<'py, T: ReadResult<'py>> ReadResult<'py> for Option<T> {
    fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        if value.is_none() {
            Ok(None)
        } else {
            T::read(value).map(Some)
        }
    }
}

impl<'py, T: ReadResult<'py>> ReadResult<'py> for Vec<T> {
    fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        value.try_iter()?.map(|item| T::read(&item?)).collect()
    }
}

impl<'py, A: ReadResult<'py>, B: ReadResult<'py>> ReadResult<'py> for (A, B) {
    fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        let (first, second): (Bound<'py, PyAny>, Bound<'py, PyAny>) = value.extract()?;
        Ok((A::read(&first)?, B::read(&second)?))
    }
}

macro_rules! enums {
    ($($kind:ty),+ $(,)?) => {$ (
        impl<'py> ReadResult<'py> for $kind {
            fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
                let text: String = field(value, "value")?.extract()?;
                Self::from_value(&text).ok_or_else(|| PyValueError::new_err(
                    format!("invalid {} value: {text}", stringify!($kind))
                ))
            }
        }
    )+};
}

enums!(
    BuildStatus,
    ExecutionStatus,
    ExecutionPhase,
    HookPhase,
    AuditRunScope,
    AttachedAuditTargetKind,
    AuditEvaluationMode,
    LifeCycleEventKind,
    AuditAttachmentKind,
    AuditSeverity,
    AuditOutcome,
    SkipMode,
    ExecutionResourceKind,
    SqlTestOutcome,
    ItemKind,
    CancellationOutcome,
    MicrobatchLimitAction,
    ScenarioLocalRunStatus,
);

macro_rules! record {
    ($kind:ident { $($name:ident),+ $(,)? }) => {
        impl<'py> ReadResult<'py> for $kind {
            fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
                Ok(Self { $($name: ReadResult::read(&field(value, stringify!($name))?)?,)+ })
            }
        }
    };
    ($kind:ident<V> { $($name:ident),+ $(,)? }) => {
        impl<'py> ReadResult<'py> for $kind<Bound<'py, PyAny>> {
            fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
                Ok(Self { $($name: ReadResult::read(&field(value, stringify!($name))?)?,)+ })
            }
        }
    };
}

record!(BuildExecutionTimings {
    connection_preparation_seconds,
    schema_preparation_seconds,
    execution_seconds,
});
record!(LifeCycleEvent { kind, content });
record!(HookExecutionResult {
    phase,
    index,
    hook_type,
    label,
    status,
    skip_mode,
    skip_reason,
    error_message,
});
record!(MicrobatchAccountingInterval {
    partition_start,
    partition_end,
    accounting_status,
    fingerprint_status,
    model_version_hash,
    completion_type,
    event_id,
});
record!(ModelExecutionResult<V> {
    model_name, status, failed_phase, staging_relation, promoted_relation, duration_ms,
    batch_count, batch_size, rows_affected, cursor_range_start, cursor_range_end, cursor_type,
    cursor_grain, empty_cursor_inputs, waiting_on_empty_inputs, future_cursor_safety,
    maximum_start_safety, audit_results, warning_messages, lifecycle_events, hook_results,
    skip_mode, skip_reason, error_code, error_help, error_message, failed_sql,
    microbatch_run_type, microbatch_strategy, microbatch_plan_reason,
    microbatch_recovery_batch_count, microbatch_known_gap_count,
    microbatch_unaccounted_interval_count, microbatch_synthetic_completion_count,
    microbatch_unknown_fingerprint_count, microbatch_contiguous_frontier,
    microbatch_unaccounted_partition_policy, microbatch_replay_requirement_id,
    microbatch_required_model_version_hash, microbatch_physical_generation_id,
    microbatch_concurrent_enabled, microbatch_batch_concurrency, microbatch_global_concurrency,
    microbatch_replay_requirement_state, microbatch_accounting_intervals,
    microbatch_limit, microbatch_limit_count, microbatch_limit_action, microbatch_limit_warning,
});
record!(SeedExecutionResult {
    seed_name,
    status,
    duration_ms,
    lifecycle_events,
    error_code,
    error_help,
    error_message,
    warning_messages,
});
record!(FunctionExecutionResult {
    function_name,
    status,
    function_kind,
    duration_ms,
    error_code,
    error_help,
    error_message,
    warning_messages,
    lifecycle_events,
});
record!(SqlTestDifferenceSample { values });
record!(SqlTestColumnDifference {
    name,
    actual,
    expected
});
record!(StepResult {
    model_name,
    outcome,
    actual_row_count,
    expected_row_count,
    mismatched_row_count,
    unexpected_row_count,
    missing_row_count,
    unexpected_samples,
    missing_samples,
    column_differences,
    error_code,
    error_help,
    error_message,
});
record!(ItemTimeoutReport {
    kind,
    name,
    limit,
    source,
    elapsed_seconds,
    query_ids,
    cancellation,
    cancellation_detail,
});
record!(SqlTestExecutionResult<V> {
    test_name, outcome, source_path, block_index, parent_name, case_name, case_index,
    case_fingerprint, parameter_schema, parameter_values, step_results, error_code, error_help,
    error_message, real_model_count, timeout,
});
record!(BuildExecutionResult<V> {
    status, model_results, seed_results, function_results, load_results, test_results,
    source_audit_results, end_audit_results, success_count, failure_count, timeout_count,
    skipped_count, warning_count, insufficient_count, audit_result_projection_attempted_count,
    audit_result_projection_written_count, audit_result_projection_failed_count, timings,
});

fn mapping<'py>(value: &Bound<'py, PyAny>) -> PyResult<Vec<(String, Bound<'py, PyAny>)>> {
    Vec::read(&value.call_method0("items")?)
}

record!(ScenarioExpectedExpectationExecutionResult {
    model_name,
    status,
    actual_row_count,
    expected_row_count,
    mismatched_row_count,
    missing_row_count,
    error_code,
    error_help,
    error_message,
});
record!(ScenarioAssertionExpectationExecutionResult<V> {
    name, status, failing_row_count, sample_rows, error_code, error_help, error_message,
});

impl<'py> ReadResult<'py> for ScenarioRunResult<Bound<'py, PyAny>> {
    fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        let relation_map = field(value, "relation_map")?;
        Ok(Self {
            scenario_name: ReadResult::read(&field(value, "scenario_name")?)?,
            status: ReadResult::read(&field(value, "status")?)?,
            retained: ReadResult::read(&field(value, "retained")?)?,
            local_status: ReadResult::read(&field(value, "local_status")?)?,
            local_duckdb_path: ReadResult::read(&field(value, "local_duckdb_path")?)?,
            retained_relations: if relation_map.is_none() {
                None
            } else {
                Some(
                    field(&relation_map, "artifacts")?
                        .try_iter()?
                        .map(|artifact| {
                            let artifact = artifact?;
                            let identity = field(&artifact, "identity")?;
                            let kind = enum_from_value(
                                value.py(),
                                RecordId::ScenarioArtifactKind,
                                field(&identity, "kind")?,
                            )?;
                            Ok(RetainedRelation {
                                kind: field(&kind, "value")?.extract()?,
                                logical_name: field(&identity, "logical_name")?.extract()?,
                                physical_name: field(&artifact, "physical_name")?.extract()?,
                            })
                        })
                        .collect::<PyResult<_>>()?,
                )
            },
            seed_results: ReadResult::read(&field(value, "seed_results")?)?,
            function_results: ReadResult::read(&field(value, "function_results")?)?,
            model_results: ReadResult::read(&field(value, "model_results")?)?,
            expected_results: ReadResult::read(&field(value, "expected_results")?)?,
            assertion_results: ReadResult::read(&field(value, "assertion_results")?)?,
            error_code: ReadResult::read(&field(value, "error_code")?)?,
            error_help: ReadResult::read(&field(value, "error_help")?)?,
            error_message: ReadResult::read(&field(value, "error_message")?)?,
            duration_ms: ReadResult::read(&field(value, "duration_ms")?)?,
        })
    }
}

impl<'py> ReadResult<'py> for AuditExecutionResult<Bound<'py, PyAny>> {
    fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        let meta = field(value, "audit_meta")?;
        Ok(Self {
            audit_name: ReadResult::read(&field(value, "audit_name")?)?,
            audit_definition_name: ReadResult::read(&field(value, "audit_definition_name")?)?,
            attachment_kind: ReadResult::read(&field(value, "attachment_kind")?)?,
            severity: ReadResult::read(&field(value, "severity")?)?,
            outcome: ReadResult::read(&field(value, "outcome")?)?,
            row_count: ReadResult::read(&field(value, "row_count")?)?,
            executed_sql: ReadResult::read(&field(value, "executed_sql")?)?,
            run_scope_phase: ReadResult::read(&field(value, "run_scope_phase")?)?,
            attached_target_kind: ReadResult::read(&field(value, "attached_target_kind")?)?,
            attached_target_name: ReadResult::read(&field(value, "attached_target_name")?)?,
            attached_column_name: ReadResult::read(&field(value, "attached_column_name")?)?,
            reused: ReadResult::read(&field(value, "reused")?)?,
            evaluation_mode: ReadResult::read(&field(value, "evaluation_mode")?)?,
            measured_value: ReadResult::read(&field(value, "measured_value")?)?,
            sample_count: ReadResult::read(&field(value, "sample_count")?)?,
            sample_unit: ReadResult::read(&field(value, "sample_unit")?)?,
            minimum_samples: ReadResult::read(&field(value, "minimum_samples")?)?,
            thresholds: ReadResult::read(&field(value, "thresholds")?)?,
            evidence_rows: field(value, "evidence_rows")?
                .try_iter()?
                .map(|row| mapping(&row?))
                .collect::<PyResult<_>>()?,
            evidence_truncated: ReadResult::read(&field(value, "evidence_truncated")?)?,
            evidence_error: ReadResult::read(&field(value, "evidence_error")?)?,
            evidence_sql: ReadResult::read(&field(value, "evidence_sql")?)?,
            audit_description: ReadResult::read(&field(value, "audit_description")?)?,
            execution_error: ReadResult::read(&field(value, "execution_error")?)?,
            audit_meta: if meta.is_none() {
                None
            } else {
                Some(mapping(&meta)?)
            },
            skip_reason: ReadResult::read(&field(value, "skip_reason")?)?,
        })
    }
}

impl<'py> ReadResult<'py> for LoadExecutionResult<Bound<'py, PyAny>> {
    fn read(value: &Bound<'py, PyAny>) -> PyResult<Self> {
        Ok(Self {
            source_name: ReadResult::read(&field(value, "source_name")?)?,
            loader_name: ReadResult::read(&field(value, "loader_name")?)?,
            status: ReadResult::read(&field(value, "status")?)?,
            target: ReadResult::read(&field(value, "target")?)?,
            resource_kind: ReadResult::read(&field(value, "resource_kind")?)?,
            staging_relation: ReadResult::read(&field(value, "staging_relation")?)?,
            rows_loaded: ReadResult::read(&field(value, "rows_loaded")?)?,
            duration_ms: ReadResult::read(&field(value, "duration_ms")?)?,
            lifecycle_events: ReadResult::read(&field(value, "lifecycle_events")?)?,
            skip_mode: ReadResult::read(&field(value, "skip_mode")?)?,
            skip_reason: ReadResult::read(&field(value, "skip_reason")?)?,
            error_message: ReadResult::read(&field(value, "error_message")?)?,
            result_payload: ReadResult::read(&field(value, "result_payload")?)?,
            result_metadata: mapping(&field(value, "result_metadata")?)?,
            result_materialized: ReadResult::read(&field(value, "result_materialized")?)?,
            warning_messages: ReadResult::read(&field(value, "warning_messages")?)?,
        })
    }
}
