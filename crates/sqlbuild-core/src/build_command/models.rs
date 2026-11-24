//! Plain plan facts, gates and outputs of the build command.

use crate::build_command::errors::BuildCommandError;

/// One planned model migration, as the safety gates read it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelMigrationFact {
    pub model_name: String,
    /// The origin's qualified name, or its name.
    pub origin_label: String,
    pub decision: String,
    pub blocks_build: bool,
    pub origin_hidden: bool,
    pub origin_missing: bool,
    /// The target's missing-origin policy asks for confirmation.
    pub confirm_missing_origin: bool,
}

/// One planned column rename, as the safety gates read it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColumnMigrationFact {
    pub model_name: String,
    pub destination_column: String,
    pub origin_column: String,
    pub decision: String,
    pub decision_label: String,
    pub blocks_build: bool,
    pub origin_missing: bool,
    pub confirm_missing_origin: bool,
}

/// An error-severity plan warning.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanWarningFact {
    pub code: Option<String>,
    pub message: String,
}

/// A selected model that couldn't be planned, with its error.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelErrorFact {
    pub code: Option<String>,
    pub message: String,
    pub help: Option<String>,
}

/// A snapshot model planned for a full refresh.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SnapshotRefreshFact {
    pub name: String,
    /// The snapshot records an `observed_at` column.
    pub historical: bool,
    pub model_policy: Option<String>,
}

/// A planned permanent-to-transient table-type downgrade.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TableTypeFact {
    pub model_name: String,
    pub policy: String,
}

/// A planned time-travel retention decrease.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RetentionFact {
    pub model_names: Vec<String>,
    pub policy: String,
}

/// Everything the plan safety gates read from one plan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanSafetyFacts {
    pub model_migrations: Vec<ModelMigrationFact>,
    pub column_migrations: Vec<ColumnMigrationFact>,
    pub error_warnings: Vec<PlanWarningFact>,
    pub model_errors: Vec<ModelErrorFact>,
    pub executable_model_count: usize,
    pub snapshot_refreshes: Vec<SnapshotRefreshFact>,
    pub table_type_downgrades: Vec<TableTypeFact>,
    pub retention_decreases: Vec<RetentionFact>,
}

/// Pre-confirmed gates and the guidance shown when one needs a terminal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanSafetyGates {
    pub allow_missing_migration_origin: bool,
    pub allow_snapshot_full_refresh: bool,
    pub allow_table_type_downgrade: bool,
    pub allow_retention_decrease: bool,
    pub missing_origin_help: String,
    pub snapshot_full_refresh_help: String,
    pub table_type_downgrade_help: String,
    pub retention_decrease_help: String,
    pub execution_limit_note: String,
}

/// Project snapshot full-refresh policies.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SnapshotPolicies {
    pub historical: String,
    pub current_state: String,
}

/// A target's model-count limit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelLimit {
    pub target_name: Option<String>,
    pub max_models: Option<i64>,
    pub remediation: Option<String>,
}

/// A target's duration limit against what its adapter can enforce.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DurationLimit {
    pub target_name: Option<String>,
    pub adapter_name: String,
    pub configures_max_duration: bool,
    pub max_duration_seconds: Option<i64>,
    pub adapter_limit_seconds: Option<i64>,
    pub remediation: Option<String>,
}

/// A typed confirmation one gate asks for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Confirmation {
    pub warning: String,
    pub expected: String,
    /// Raised when the run cannot ask.
    pub refusal: BuildCommandError,
    /// Raised when the answer does not match.
    pub cancellation: BuildCommandError,
}

/// One plan safety gate, in the order the gates apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyStep {
    Refuse(BuildCommandError),
    Confirm(Confirmation),
}

impl PolicyStep {
    /// The refusal this step raises, if it is one.
    #[must_use]
    pub fn refusal(&self) -> Option<&BuildCommandError> {
        match self {
            Self::Refuse(error) => Some(error),
            Self::Confirm(_) => None,
        }
    }

    /// The confirmation this step asks for, if it is one.
    #[must_use]
    pub fn confirmation(&self) -> Option<&Confirmation> {
        match self {
            Self::Refuse(_) => None,
            Self::Confirm(confirmation) => Some(confirmation),
        }
    }
}

/// Final monotonic phase durations for verbose build output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhaseTimings {
    pub compile_seconds: Option<f64>,
    pub planning_seconds: Option<f64>,
    pub connection_preparation_seconds: Option<f64>,
    pub schema_preparation_seconds: Option<f64>,
    pub execution_seconds: Option<f64>,
    pub cost_collection_seconds: Option<f64>,
    pub total_seconds: Option<f64>,
}

/// One selector file the selection read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectorFileFact {
    pub path: String,
    pub selector_count: usize,
}

/// Resolved, non-secret build placement, mode and selection context.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunContextFacts {
    pub command: String,
    pub run_id: Option<String>,
    pub target: Option<String>,
    pub database: Option<String>,
    pub schema: Option<String>,
    /// The warehouse and where it came from.
    pub warehouse: Option<(String, String)>,
    pub concurrency: i64,
    pub full_refresh: bool,
    /// The named jobs whose own models the build rebuilds from scratch.
    pub full_refresh_jobs: Vec<String>,
    /// Each named job whose nodes run on its own warehouse, and that warehouse.
    pub job_warehouses: Vec<(String, String)>,
    pub selected_count: usize,
    pub total_count: usize,
    /// Valid ISO `start_date` and `end_date` variables.
    pub date_vars: (Option<String>, Option<String>),
    pub selector_files: Vec<SelectorFileFact>,
}

/// One finished asset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PythonNodeRow {
    pub kind: String,
    pub node_name: String,
    pub status: String,
    pub error_message: Option<String>,
    pub skip_reason: Option<String>,
}
