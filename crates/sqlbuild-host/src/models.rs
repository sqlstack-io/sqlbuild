//! Executor results as Rust data: one type per Python result dataclass and string enum.

use std::path::PathBuf;

use sqlbuild_core::json::models::JsonInteger;

use crate::types::ResultEnum;

/// Whether a build succeeded. Mirrors `sqlbuild.executor.build.types.BuildStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildStatus {
    Success,
    Failed,
}

impl ResultEnum for BuildStatus {
    const ALL: &'static [Self] = &[Self::Success, Self::Failed];

    fn value(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
        }
    }
}

/// How one executed node ended. Mirrors `sqlbuild.executor.scheduling.types.ExecutionStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionStatus {
    Success,
    Failed,
    Skipped,
}

impl ResultEnum for ExecutionStatus {
    const ALL: &'static [Self] = &[Self::Success, Self::Failed, Self::Skipped];

    fn value(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

/// The model phase a failure happened in. Mirrors `sqlbuild.executor.run.types.ExecutionPhase`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionPhase {
    PreHook,
    Staging,
    SchemaChange,
    TypeEnforcement,
    Contract,
    Audit,
    Promotion,
    Dml,
    PostHook,
    Fingerprint,
    MicrobatchState,
    CustomMaterialization,
}

impl ResultEnum for ExecutionPhase {
    const ALL: &'static [Self] = &[
        Self::PreHook,
        Self::Staging,
        Self::SchemaChange,
        Self::TypeEnforcement,
        Self::Contract,
        Self::Audit,
        Self::Promotion,
        Self::Dml,
        Self::PostHook,
        Self::Fingerprint,
        Self::MicrobatchState,
        Self::CustomMaterialization,
    ];

    fn value(self) -> &'static str {
        match self {
            Self::PreHook => "pre_hook",
            Self::Staging => "staging",
            Self::SchemaChange => "schema_change",
            Self::TypeEnforcement => "type_enforcement",
            Self::Contract => "contract",
            Self::Audit => "audit",
            Self::Promotion => "promotion",
            Self::Dml => "dml",
            Self::PostHook => "post_hook",
            Self::Fingerprint => "fingerprint",
            Self::MicrobatchState => "microbatch_state",
            Self::CustomMaterialization => "custom_materialization",
        }
    }
}

/// Whether a hook ran before or after its model. Mirrors `sqlbuild.executor.run.types.HookPhase`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookPhase {
    PreHooks,
    PostHooks,
}

impl ResultEnum for HookPhase {
    const ALL: &'static [Self] = &[Self::PreHooks, Self::PostHooks];

    fn value(self) -> &'static str {
        match self {
            Self::PreHooks => "pre_hooks",
            Self::PostHooks => "post_hooks",
        }
    }
}

/// How a skipped node blocks its dependents. Mirrors `sqlbuild.compiler.python_nodes.types.SkipMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipMode {
    Soft,
    Hard,
}

impl ResultEnum for SkipMode {
    const ALL: &'static [Self] = &[Self::Soft, Self::Hard];

    fn value(self) -> &'static str {
        match self {
            Self::Soft => "soft",
            Self::Hard => "hard",
        }
    }
}

/// What a microbatch limit does when reached. Mirrors `sqlbuild._native.MicrobatchLimitAction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicrobatchLimitAction {
    CapFromEnd,
    CapFromStart,
    Error,
    Warn,
}

impl ResultEnum for MicrobatchLimitAction {
    const ALL: &'static [Self] = &[
        Self::CapFromEnd,
        Self::CapFromStart,
        Self::Error,
        Self::Warn,
    ];

    fn value(self) -> &'static str {
        match self {
            Self::CapFromEnd => "cap_from_end",
            Self::CapFromStart => "cap_from_start",
            Self::Error => "error",
            Self::Warn => "warn",
        }
    }
}

/// The kind of resource a load wrote. Mirrors `sqlbuild.runtime.contracts.types.ExecutionResourceKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionResourceKind {
    Source,
    Loader,
    Seed,
    Udf,
    TableFn,
    View,
    Table,
    Custom,
    Snapshot,
}

impl ResultEnum for ExecutionResourceKind {
    const ALL: &'static [Self] = &[
        Self::Source,
        Self::Loader,
        Self::Seed,
        Self::Udf,
        Self::TableFn,
        Self::View,
        Self::Table,
        Self::Custom,
        Self::Snapshot,
    ];

    fn value(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Loader => "loader",
            Self::Seed => "seed",
            Self::Udf => "udf",
            Self::TableFn => "table_fn",
            Self::View => "view",
            Self::Table => "table",
            Self::Custom => "custom",
            Self::Snapshot => "snapshot",
        }
    }
}

/// How one SQL test or step ended. Mirrors `sqlbuild.executor.testing.types.SqlTestOutcome`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqlTestOutcome {
    Pass,
    Fail,
    Error,
    Timeout,
}

impl ResultEnum for SqlTestOutcome {
    const ALL: &'static [Self] = &[Self::Pass, Self::Fail, Self::Error, Self::Timeout];

    fn value(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Error => "error",
            Self::Timeout => "timeout",
        }
    }
}

/// The kind of item a timeout stopped. Mirrors `sqlbuild.runtime.execution_limits.types.ItemKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Test,
    Scenario,
    Model,
}

impl ResultEnum for ItemKind {
    const ALL: &'static [Self] = &[Self::Test, Self::Scenario, Self::Model];

    fn value(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Scenario => "scenario",
            Self::Model => "model",
        }
    }
}

/// Whether a timed-out query was cancelled. Mirrors `sqlbuild.runtime.execution_limits.types.CancellationOutcome`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellationOutcome {
    Confirmed,
    Unknown,
    Failed,
    Unsupported,
    NotNeeded,
}

impl ResultEnum for CancellationOutcome {
    const ALL: &'static [Self] = &[
        Self::Confirmed,
        Self::Unknown,
        Self::Failed,
        Self::Unsupported,
        Self::NotNeeded,
    ];

    fn value(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Unknown => "unknown",
            Self::Failed => "failed",
            Self::Unsupported => "unsupported",
            Self::NotNeeded => "not_needed",
        }
    }
}

/// Where an audit is attached. Mirrors `sqlbuild.compiler.auditing.types.AuditAttachmentKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditAttachmentKind {
    Source,
    Seed,
    Model,
    End,
}

impl ResultEnum for AuditAttachmentKind {
    const ALL: &'static [Self] = &[Self::Source, Self::Seed, Self::Model, Self::End];

    fn value(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Seed => "seed",
            Self::Model => "model",
            Self::End => "end",
        }
    }
}

/// How a failing audit is reported. Mirrors `sqlbuild.compiler.auditing.types.AuditSeverity`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditSeverity {
    Warn,
    Error,
}

impl ResultEnum for AuditSeverity {
    const ALL: &'static [Self] = &[Self::Warn, Self::Error];

    fn value(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// How one audit ended. Mirrors `sqlbuild.compiler.auditing.types.AuditOutcome`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditOutcome {
    Pass,
    Warn,
    Error,
    Insufficient,
    Skipped,
}

impl ResultEnum for AuditOutcome {
    const ALL: &'static [Self] = &[
        Self::Pass,
        Self::Warn,
        Self::Error,
        Self::Insufficient,
        Self::Skipped,
    ];

    fn value(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Insufficient => "insufficient",
            Self::Skipped => "skipped",
        }
    }
}

/// The phases an audit runs in. Mirrors `sqlbuild.compiler.auditing.types.AuditRunScope`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditRunScope {
    Final,
    DeltaAndFinal,
}

impl ResultEnum for AuditRunScope {
    const ALL: &'static [Self] = &[Self::Final, Self::DeltaAndFinal];

    fn value(self) -> &'static str {
        match self {
            Self::Final => "final",
            Self::DeltaAndFinal => "delta_and_final",
        }
    }
}

/// The resource an attached audit checks. Mirrors `sqlbuild.compiler.compile.types.AttachedAuditTargetKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachedAuditTargetKind {
    Model,
    Source,
    Seed,
}

impl ResultEnum for AttachedAuditTargetKind {
    const ALL: &'static [Self] = &[Self::Model, Self::Source, Self::Seed];

    fn value(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Source => "source",
            Self::Seed => "seed",
        }
    }
}

/// Whether an audit counts violations or measures a value. Mirrors `sqlbuild.compiler.auditing.types.AuditEvaluationMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditEvaluationMode {
    Violations,
    Measurement,
}

impl ResultEnum for AuditEvaluationMode {
    const ALL: &'static [Self] = &[Self::Violations, Self::Measurement];

    fn value(self) -> &'static str {
        match self {
            Self::Violations => "violations",
            Self::Measurement => "measurement",
        }
    }
}

/// The kind of one lifecycle event. Mirrors `sqlbuild.adapter.contract.types.LifeCycleEventKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifeCycleEventKind {
    Sql,
    Log,
}

impl ResultEnum for LifeCycleEventKind {
    const ALL: &'static [Self] = &[Self::Sql, Self::Log];

    fn value(self) -> &'static str {
        match self {
            Self::Sql => "sql",
            Self::Log => "log",
        }
    }
}

/// The wall time of a build's preparation and execution. Mirrors `sqlbuild.executor.build.models.BuildExecutionTimings`.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildExecutionTimings {
    pub connection_preparation_seconds: Option<f64>,
    pub schema_preparation_seconds: Option<f64>,
    pub execution_seconds: Option<f64>,
}

/// One SQL statement or log line a node emitted. Mirrors `sqlbuild.adapter.contract.models.LifeCycleEvent`.
#[derive(Clone, Debug, PartialEq)]
pub struct LifeCycleEvent {
    pub kind: LifeCycleEventKind,
    pub content: String,
}

/// One pre- or post-hook of a model. Mirrors `sqlbuild.executor.run.models.HookExecutionResult`.
#[derive(Clone, Debug, PartialEq)]
pub struct HookExecutionResult {
    pub phase: HookPhase,
    pub index: i64,
    pub hook_type: String,
    pub label: String,
    pub status: ExecutionStatus,
    pub skip_mode: Option<SkipMode>,
    pub skip_reason: Option<String>,
    pub error_message: Option<String>,
}

/// One partition interval a microbatch run accounted for. Mirrors `sqlbuild.executor.run.models.MicrobatchAccountingInterval`.
#[derive(Clone, Debug, PartialEq)]
pub struct MicrobatchAccountingInterval {
    pub partition_start: String,
    pub partition_end: String,
    pub accounting_status: String,
    pub fingerprint_status: String,
    pub model_version_hash: Option<String>,
    pub completion_type: Option<String>,
    pub event_id: Option<String>,
}

/// A measured number without coercing integer values to floating point.
#[derive(Clone, Debug, PartialEq)]
pub enum MeasuredValue {
    Integer(JsonInteger),
    Float(f64),
}

/// One executed audit. Mirrors `sqlbuild.executor.auditing.models.AuditExecutionResult`; `V` is a host value.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditExecutionResult<V> {
    pub audit_name: String,
    pub audit_definition_name: String,
    pub attachment_kind: AuditAttachmentKind,
    pub severity: AuditSeverity,
    pub outcome: AuditOutcome,
    pub row_count: i64,
    pub executed_sql: String,
    pub run_scope_phase: AuditRunScope,
    pub attached_target_kind: Option<AttachedAuditTargetKind>,
    pub attached_target_name: Option<String>,
    pub attached_column_name: Option<String>,
    pub reused: bool,
    pub evaluation_mode: AuditEvaluationMode,
    pub measured_value: Option<MeasuredValue>,
    pub sample_count: Option<i64>,
    pub sample_unit: Option<String>,
    pub minimum_samples: Option<i64>,
    pub thresholds: Option<V>,
    pub evidence_rows: Vec<Vec<(String, V)>>,
    pub evidence_truncated: bool,
    pub evidence_error: Option<String>,
    pub evidence_sql: Option<String>,
    pub audit_description: Option<String>,
    pub execution_error: Option<String>,
    pub audit_meta: Option<Vec<(String, V)>>,
    pub skip_reason: Option<String>,
}

/// One executed model. Mirrors `sqlbuild.executor.run.models.ModelExecutionResult`; `V` is a host value.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelExecutionResult<V> {
    pub model_name: String,
    pub status: ExecutionStatus,
    pub failed_phase: Option<ExecutionPhase>,
    pub staging_relation: Option<String>,
    pub promoted_relation: Option<String>,
    pub duration_ms: Option<i64>,
    pub batch_count: Option<i64>,
    pub batch_size: Option<String>,
    pub rows_affected: Option<i64>,
    pub cursor_range_start: Option<String>,
    pub cursor_range_end: Option<String>,
    pub cursor_type: Option<String>,
    pub cursor_grain: Option<String>,
    pub empty_cursor_inputs: Vec<String>,
    pub waiting_on_empty_inputs: bool,
    pub future_cursor_safety: Option<V>,
    pub maximum_start_safety: Option<V>,
    pub audit_results: Vec<AuditExecutionResult<V>>,
    pub warning_messages: Vec<String>,
    pub lifecycle_events: Vec<LifeCycleEvent>,
    pub hook_results: Vec<HookExecutionResult>,
    pub skip_mode: Option<SkipMode>,
    pub skip_reason: Option<String>,
    pub error_code: Option<String>,
    pub error_help: Option<String>,
    pub error_message: Option<String>,
    pub failed_sql: Option<String>,
    pub microbatch_run_type: Option<String>,
    pub microbatch_strategy: Option<String>,
    pub microbatch_plan_reason: Option<String>,
    pub microbatch_recovery_batch_count: i64,
    pub microbatch_known_gap_count: i64,
    pub microbatch_unaccounted_interval_count: i64,
    pub microbatch_synthetic_completion_count: i64,
    pub microbatch_unknown_fingerprint_count: i64,
    pub microbatch_contiguous_frontier: Option<String>,
    pub microbatch_unaccounted_partition_policy: Option<String>,
    pub microbatch_replay_requirement_id: Option<String>,
    pub microbatch_required_model_version_hash: Option<String>,
    pub microbatch_physical_generation_id: Option<String>,
    pub microbatch_concurrent_enabled: bool,
    pub microbatch_batch_concurrency: i64,
    pub microbatch_global_concurrency: i64,
    pub microbatch_replay_requirement_state: Option<String>,
    pub microbatch_accounting_intervals: Vec<MicrobatchAccountingInterval>,
    pub microbatch_limit: Option<i64>,
    pub microbatch_limit_count: Option<i64>,
    pub microbatch_limit_action: Option<MicrobatchLimitAction>,
    pub microbatch_limit_warning: Option<String>,
}

/// One loaded seed. Mirrors `sqlbuild.executor.build.models.SeedExecutionResult`.
#[derive(Clone, Debug, PartialEq)]
pub struct SeedExecutionResult {
    pub seed_name: String,
    pub status: ExecutionStatus,
    pub duration_ms: Option<i64>,
    pub lifecycle_events: Vec<LifeCycleEvent>,
    pub error_code: Option<String>,
    pub error_help: Option<String>,
    pub error_message: Option<String>,
    pub warning_messages: Vec<String>,
}

/// One created function. Mirrors `sqlbuild.executor.build.models.FunctionExecutionResult`.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionExecutionResult {
    pub function_name: String,
    pub status: ExecutionStatus,
    pub function_kind: String,
    pub duration_ms: Option<i64>,
    pub error_code: Option<String>,
    pub error_help: Option<String>,
    pub error_message: Option<String>,
    pub warning_messages: Vec<String>,
    pub lifecycle_events: Vec<LifeCycleEvent>,
}

/// One source or loader load. Mirrors `sqlbuild.executor.load.models.LoadExecutionResult`; `V` is a host value.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadExecutionResult<V> {
    pub source_name: String,
    pub loader_name: String,
    pub status: ExecutionStatus,
    pub target: String,
    pub resource_kind: ExecutionResourceKind,
    pub staging_relation: Option<String>,
    pub rows_loaded: i64,
    pub duration_ms: Option<i64>,
    pub lifecycle_events: Vec<LifeCycleEvent>,
    pub skip_mode: Option<SkipMode>,
    pub skip_reason: Option<String>,
    pub error_message: Option<String>,
    pub result_payload: Option<V>,
    pub result_metadata: Vec<(String, V)>,
    pub result_materialized: Option<bool>,
    pub warning_messages: Vec<String>,
}

/// One sampled row a SQL test step found missing or unexpected. Mirrors `sqlbuild.executor.testing.models.SqlTestDifferenceSample`.
#[derive(Clone, Debug, PartialEq)]
pub struct SqlTestDifferenceSample {
    pub values: Vec<(String, String)>,
}

/// One column whose actual and expected types differ. Mirrors `sqlbuild.executor.testing.models.SqlTestColumnDifference`.
#[derive(Clone, Debug, PartialEq)]
pub struct SqlTestColumnDifference {
    pub name: String,
    pub actual: String,
    pub expected: String,
}

/// One model step of a SQL test. Mirrors `sqlbuild.executor.testing.models.StepResult`.
#[derive(Clone, Debug, PartialEq)]
pub struct StepResult {
    pub model_name: String,
    pub outcome: SqlTestOutcome,
    pub actual_row_count: i64,
    pub expected_row_count: i64,
    pub mismatched_row_count: i64,
    pub unexpected_row_count: Option<i64>,
    pub missing_row_count: Option<i64>,
    pub unexpected_samples: Vec<SqlTestDifferenceSample>,
    pub missing_samples: Vec<SqlTestDifferenceSample>,
    pub column_differences: Vec<SqlTestColumnDifference>,
    pub error_code: Option<String>,
    pub error_help: Option<String>,
    pub error_message: Option<String>,
}

/// The timeout that stopped one item. Mirrors `sqlbuild.runtime.execution_limits.models.ItemTimeoutReport`.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemTimeoutReport {
    pub kind: ItemKind,
    pub name: String,
    pub limit: String,
    pub source: String,
    pub elapsed_seconds: f64,
    pub query_ids: Vec<String>,
    pub cancellation: CancellationOutcome,
    pub cancellation_detail: Option<String>,
}

/// One executed SQL test case. Mirrors `sqlbuild.executor.testing.models.SqlTestExecutionResult`; `V` is a host value.
#[derive(Clone, Debug, PartialEq)]
pub struct SqlTestExecutionResult<V> {
    pub test_name: String,
    pub outcome: SqlTestOutcome,
    pub source_path: Option<PathBuf>,
    pub block_index: Option<i64>,
    pub parent_name: Option<String>,
    pub case_name: Option<String>,
    pub case_index: Option<i64>,
    pub case_fingerprint: Option<String>,
    pub parameter_schema: Vec<V>,
    pub parameter_values: Vec<(String, V)>,
    pub step_results: Vec<StepResult>,
    pub error_code: Option<String>,
    pub error_help: Option<String>,
    pub error_message: Option<String>,
    pub real_model_count: Option<i64>,
    pub timeout: Option<ItemTimeoutReport>,
}

/// Everything one build executed, as the executor returns it. Mirrors `sqlbuild.executor.build.models.BuildExecutionResult`; `V` is a host value.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildExecutionResult<V> {
    pub status: BuildStatus,
    pub model_results: Vec<ModelExecutionResult<V>>,
    pub seed_results: Vec<SeedExecutionResult>,
    pub function_results: Vec<FunctionExecutionResult>,
    pub load_results: Vec<LoadExecutionResult<V>>,
    pub test_results: Vec<SqlTestExecutionResult<V>>,
    pub source_audit_results: Vec<AuditExecutionResult<V>>,
    pub end_audit_results: Vec<AuditExecutionResult<V>>,
    pub success_count: i64,
    pub failure_count: i64,
    pub timeout_count: i64,
    pub skipped_count: i64,
    pub warning_count: i64,
    pub insufficient_count: i64,
    pub audit_result_projection_attempted_count: i64,
    pub audit_result_projection_written_count: i64,
    pub audit_result_projection_failed_count: i64,
    pub timings: BuildExecutionTimings,
}

/// The outcome of a local scenario replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioLocalRunStatus {
    Pass,
    Fail,
    Error,
    Skip,
}

impl ResultEnum for ScenarioLocalRunStatus {
    const ALL: &'static [Self] = &[Self::Pass, Self::Fail, Self::Error, Self::Skip];

    fn value(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Error => "ERROR",
            Self::Skip => "SKIP",
        }
    }
}

/// The executor's comparison outcome; absent counts mean the comparison did not run.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioExpectedExpectationExecutionResult {
    pub model_name: String,
    pub status: ExecutionStatus,
    pub actual_row_count: Option<i64>,
    pub expected_row_count: Option<i64>,
    pub mismatched_row_count: Option<i64>,
    pub missing_row_count: Option<i64>,
    pub error_code: Option<String>,
    pub error_help: Option<String>,
    pub error_message: Option<String>,
}

/// The executor's zero-row assertion outcome; only warehouse cell values remain host-owned.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioAssertionExpectationExecutionResult<V> {
    pub name: String,
    pub status: ExecutionStatus,
    pub failing_row_count: Option<i64>,
    pub sample_rows: Vec<Vec<V>>,
    pub error_code: Option<String>,
    pub error_help: Option<String>,
    pub error_message: Option<String>,
}

/// The completed scenario fields consumed by command reporting; runtime artifact publication stays at the executor callback edge.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioRunResult<V> {
    pub scenario_name: String,
    pub status: ExecutionStatus,
    pub retained: bool,
    pub local_status: Option<ScenarioLocalRunStatus>,
    pub local_duckdb_path: Option<PathBuf>,
    pub retained_relations: Option<Vec<sqlbuild_core::scenario_command::models::RetainedRelation>>,
    pub seed_results: Vec<SeedExecutionResult>,
    pub function_results: Vec<FunctionExecutionResult>,
    pub model_results: Vec<ModelExecutionResult<V>>,
    pub expected_results: Vec<ScenarioExpectedExpectationExecutionResult>,
    pub assertion_results: Vec<ScenarioAssertionExpectationExecutionResult<V>>,
    pub error_code: Option<String>,
    pub error_help: Option<String>,
    pub error_message: Option<String>,
    pub duration_ms: Option<f64>,
}
