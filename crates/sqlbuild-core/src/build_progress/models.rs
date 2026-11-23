//! Plain data of executor results as build progress output renders them.

/// The coded error recorded on one result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ErrorText {
    pub code: Option<String>,
    pub message: Option<String>,
    pub help: Option<String>,
}

/// Why a result was skipped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkipText {
    pub mode: Option<String>,
    pub reason: Option<String>,
}

/// One lifecycle event of a model or load run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleEventText {
    Sql(String),
    Log(String),
}

/// One audit result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuditResultText {
    pub audit_name: String,
    pub attached_column_name: Option<String>,
    pub attached_target_name: Option<String>,
    pub run_scope_phase: String,
    pub outcome: String,
    pub row_count: i64,
    pub reused: bool,
    pub executed_sql: Option<String>,
    /// The audit is evaluated as a measurement instead of violation rows.
    pub measurement: bool,
    /// `str(measured_value)`.
    pub measured_value: String,
    pub execution_error: Option<String>,
    /// Why the audit did not run, when its outcome is `skipped`.
    pub skip_reason: Option<String>,
    /// An error-severity failure the build let through.
    pub ignored: bool,
}

/// One pre- or post-hook result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HookResultText {
    pub pre_hook: bool,
    pub hook_type: String,
    pub label: String,
    pub status: String,
    pub skip: SkipText,
}

/// One model result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelResultText {
    pub name: String,
    pub status: String,
    pub failed_phase: Option<String>,
    pub staging_relation: Option<String>,
    pub duration_ms: Option<f64>,
    pub batch_count: Option<i64>,
    pub batch_size: Option<String>,
    pub rows_affected: Option<i64>,
    /// The displayed first and inclusive last cursor bound, when both are known.
    pub cursor_range: Option<(String, String)>,
    pub empty_cursor_inputs: Vec<String>,
    pub waiting_on_empty_inputs: bool,
    pub recovery_batch_count: i64,
    pub synthetic_completion_count: i64,
    pub replay_requirement_state: Option<String>,
    pub audits: Vec<AuditResultText>,
    pub warnings: Vec<String>,
    pub events: Vec<LifecycleEventText>,
    pub hooks: Vec<HookResultText>,
    pub skip: SkipText,
    pub error: ErrorText,
}

/// One seed or function result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeResultText {
    pub name: String,
    /// `seed`, or the function kind.
    pub kind: String,
    pub status: String,
    pub duration_ms: Option<f64>,
    pub error: ErrorText,
    pub warnings: Vec<String>,
}

/// One source load result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadRowText {
    pub source_name: String,
    pub loader_name: String,
    pub resource_kind: String,
    pub status: String,
    pub rows_loaded: i64,
    pub duration_ms: Option<f64>,
    pub events: Vec<LifecycleEventText>,
    pub skip: SkipText,
    pub error_message: Option<String>,
    pub warnings: Vec<String>,
}

/// One expectation step of a SQL test.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestStepText {
    pub model_name: String,
    pub outcome: String,
    pub detail: String,
}

/// One SQL test result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestResultText {
    /// The test name with its parameter label.
    pub label: String,
    pub outcome: String,
    pub steps: Vec<TestStepText>,
    pub error: ErrorText,
}

/// One Python node result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PythonNodeText {
    pub name: String,
    pub kind: String,
    pub status: String,
    pub error_message: Option<String>,
    pub warnings: Vec<String>,
}

/// Every result of one build run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildResultText {
    pub failed: bool,
    pub warning_count: i64,
    pub insufficient_count: i64,
    pub models: Vec<ModelResultText>,
    pub seeds: Vec<NodeResultText>,
    pub functions: Vec<NodeResultText>,
    pub loads: Vec<LoadRowText>,
    pub tests: Vec<TestResultText>,
    pub source_audits: Vec<AuditResultText>,
    pub end_audits: Vec<AuditResultText>,
}

/// The plan facts of the model a row reports.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelRowFacts {
    pub display_type: String,
    pub annotation: String,
    pub logical_ddl: Option<String>,
}

/// Fixed geometry of the progress rows of one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgressLayout {
    pub total: usize,
    pub name_width: usize,
    pub columns: usize,
    pub verbose: bool,
}

/// One concurrent scheduler state transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SchedulerStateText {
    pub running: i64,
    pub ready: i64,
    pub waiting: i64,
    pub limit: i64,
    pub aborted: i64,
}

/// One completed warehouse statement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatementText {
    pub query_id: Option<String>,
    pub status: String,
    pub elapsed_seconds: f64,
    pub resource_type: String,
    pub resource_name: String,
    pub phase: String,
}

/// A node still running when a heartbeat line is written, with the rows it has written where known.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunningNodeText {
    pub name: String,
    pub elapsed_seconds: u64,
    pub rows_so_far: Option<i64>,
}

/// One completed microbatch of a model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchProgressText {
    pub model_name: String,
    pub batch_number: usize,
    pub batch_count: usize,
    pub window: String,
    pub rows_affected: Option<i64>,
    pub elapsed_seconds: f64,
}

/// The node shown on the live spinner row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpinnerRow {
    /// The 1-based position the running node will complete at.
    pub position: usize,
    pub resource_kind: String,
    pub name: String,
    pub message: String,
}

/// Names that set the shared name column width.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameWidthInputs {
    /// Top-level row names, with model annotations.
    pub names: Vec<String>,
    /// Expectation rows drawn as tree children.
    pub tree_child_names: Vec<String>,
}

/// A finished seed or function, or a source load.
#[derive(Clone, Copy, Debug)]
pub enum ResourceResult<'a> {
    Node(&'a NodeResultText),
    Load(&'a LoadRowText),
}
