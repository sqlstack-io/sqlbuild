//! The lifecycle facts progress projection reads, and the invocation projection state.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::progress::types::DifferenceSample;

/// The payload fields of one lifecycle event that progress projection reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgressPayload {
    /// `resource_name`, when it is a string.
    pub resource_name: Option<String>,
    /// `resource_kind`, when it is a string.
    pub resource_kind: Option<String>,
    /// `duration_ms`, when it is a number.
    pub duration_ms: Option<f64>,
    /// `attempt_number`, when it is an integral number.
    pub attempt_number: Option<i64>,
    /// `operation_name`, when it is a string.
    pub operation_name: Option<String>,
    /// `metadata.attempt_number`, when it is an `int` other than `bool`.
    pub metadata_attempt_number: Option<i64>,
    /// `metadata.item_count`, when it is an `int` other than `bool`.
    pub metadata_item_count: Option<i64>,
    /// `metadata.unknown_count`, when it is an `int` other than `bool`.
    pub metadata_unknown_count: Option<i64>,
    /// `error_type`, when it is a string.
    pub error_type: Option<String>,
    /// `failed_attempt_number`, when it is an `int` other than `bool`.
    pub failed_attempt_number: Option<i64>,
    /// `next_attempt_number`, when it is an `int` other than `bool`.
    pub next_attempt_number: Option<i64>,
    /// `delay_ms`, when it is an `int` other than `bool`.
    pub delay_ms: Option<f64>,
    /// `query_id`, when it is a string.
    pub query_id: Option<String>,
    /// `phase`, when it is a string.
    pub phase: Option<String>,
    /// The display text of `intent`, `execute` when absent.
    pub intent: String,
    /// The display text of `statement_kind`, `UNKNOWN` when absent.
    pub statement_kind: String,
}

/// The correlations and payload of one lifecycle event that progress projection reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgressEvent {
    pub event_id: String,
    pub event_type: String,
    pub resource_id: Option<String>,
    pub resource_attempt_id: Option<String>,
    pub operation_id: Option<String>,
    pub statement_id: Option<String>,
    pub payload: ProgressPayload,
}

/// How the progress stream renders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProgressSettings {
    pub use_color: bool,
    pub debug: bool,
    /// Whether the stream is a terminal, where live owners draw transient rows instead.
    pub is_tty: bool,
}

/// The invocation-scoped projection of canonical lifecycle facts onto persistent progress lines.
#[derive(Clone, Debug, Default)]
pub struct ProgressProjection {
    pub(crate) settings: ProgressSettings,
    pub(crate) seen_event_ids: HashSet<String>,
    pub(crate) resource_ordinals: HashMap<String, i64>,
    pub(crate) resource_total: i64,
    /// Attempt identifiers in first-seen order, which `close` renders in.
    pub(crate) attempt_order: Vec<String>,
    pub(crate) attempts_by_id: HashMap<String, ProgressEvent>,
    pub(crate) pending_terminal_ids_by_resource: HashMap<String, VecDeque<String>>,
    pub(crate) resource_ids_by_name: HashMap<String, HashSet<String>>,
    pub(crate) rendered_terminal_ids: HashSet<String>,
    pub(crate) operation_starts: HashMap<String, ProgressEvent>,
    pub(crate) quiet_operation_ids: HashSet<String>,
    pub(crate) enriched_resource_names: HashSet<String>,
    /// Resources that never start, such as an audit not run, so they show only their terminal row.
    pub(crate) unstarted_resource_names: HashSet<String>,
    /// Audit outcomes by attempt, which the audit's own terminal row shows.
    pub(crate) audit_outcomes: HashMap<String, String>,
    pub(crate) resource_names: HashMap<String, String>,
    pub(crate) statement_starts: HashMap<String, ProgressEvent>,
    pub(crate) statement_query_ids: HashMap<String, String>,
    pub(crate) statements: StatementSummary,
}

/// The warehouse statements an invocation finished: how many of each leading keyword, and their query identifiers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatementSummary {
    pub by_kind: BTreeMap<String, u64>,
    pub query_ids: Vec<String>,
}

/// Warehouse-reported time of the statements found in query history.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StatementTimings {
    pub compile_ms: f64,
    pub execution_ms: f64,
    /// How many of the summary's query identifiers query history returned.
    pub timed_statements: u64,
}

/// What the warehouse reports about the time of an invocation's statements.
#[derive(Clone, Debug, PartialEq)]
pub enum StatementTiming {
    Reported(StatementTimings),
    /// The adapter, by name, keeps no per-statement compile time.
    NotReported(String),
    /// Reading query history failed, for this reason.
    Failed(String),
}

impl ProgressProjection {
    /// An empty projection rendering with `settings`.
    #[must_use]
    pub fn new(settings: ProgressSettings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }

    /// Add display order metadata without changing canonical facts.
    pub fn configure_resources(&mut self, ordinals: Vec<(String, i64)>, total: i64) {
        for (name, ordinal) in ordinals {
            self.enriched_resource_names.insert(name.clone());
            self.resource_ordinals.insert(name, ordinal);
        }
        self.resource_total = self.resource_total.max(total);
    }

    /// Declare that a result callback will enrich this resource's terminal.
    pub fn expect_resource_enrichment(&mut self, resource_name: &str) {
        self.enriched_resource_names
            .insert(resource_name.to_owned());
    }

    /// Declare that this resource never starts, so its terminal row is its only row.
    pub fn expect_no_start(&mut self, resource_name: &str) {
        self.unstarted_resource_names
            .insert(resource_name.to_owned());
    }

    /// Record how one audit attempt ended, before its terminal row renders.
    pub fn record_audit_outcome(&mut self, resource_attempt_id: &str, outcome: &str) {
        self.audit_outcomes
            .insert(resource_attempt_id.to_owned(), outcome.to_owned());
    }

    /// The warehouse statements finished so far.
    #[must_use]
    pub fn statement_summary(&self) -> &StatementSummary {
        &self.statements
    }

    /// Whether a visible operation with this name currently owns progress.
    #[must_use]
    pub fn is_operation_active(&self, operation_name: &str) -> bool {
        self.operation_starts
            .values()
            .any(|event| event.payload.operation_name.as_deref() == Some(operation_name))
    }
}

/// The fields of one SQL test chain step that its expectation row reports.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpectationStep {
    pub model_name: String,
    /// The outcome value: `pass`, `fail` or `error`.
    pub outcome: String,
    pub actual_row_count: i64,
    pub expected_row_count: i64,
    pub mismatched_row_count: i64,
    pub unexpected_row_count: Option<i64>,
    pub missing_row_count: Option<i64>,
    pub unexpected_samples: Vec<DifferenceSample>,
    pub missing_samples: Vec<DifferenceSample>,
    /// Differing columns as name, actual and expected value.
    pub column_differences: Vec<(String, String, String)>,
}

/// One warehouse inspection read reported in verbose planning output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InspectionRecord {
    pub sql: String,
    pub elapsed_seconds: f64,
    pub row_count: Option<i64>,
    pub error: Option<String>,
}

/// The parsed options that decide whether a command writes machine output to stdout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputModeFacts {
    pub json: bool,
    pub command: Option<String>,
    pub lineage_format: Option<String>,
    pub query_format: Option<String>,
    /// The arguments after `sqb dbt <command>`, where `--json` selects JSON output.
    pub dbt_args: Vec<String>,
}
