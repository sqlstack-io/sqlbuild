//! A planned model as its materialization labels read it.

/// The plan-entry fields materialization labels read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelLabelFacts {
    pub materialization_type: String,
    pub action: String,
    pub custom_materialization_name: Option<String>,
    pub incremental_strategy: Option<String>,
    pub incremental_mode: Option<String>,
    pub cursor_type: Option<String>,
    pub snapshot_strategy: Option<String>,
    /// The snapshot records an `observed_at` column.
    pub historical: bool,
    pub historical_input: Option<String>,
}

/// A cursor bound value parsed as an ISO date or datetime.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedBound {
    /// The value's calendar date as `YYYY-MM-DD`.
    pub date: String,
    /// Hour, minute, second and microsecond are all zero.
    pub midnight: bool,
}

/// A cursor's type, strategy, declared grain and its inputs' grains, as the replay grain reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimestampGrainFacts {
    pub cursor_type: Option<String>,
    pub microbatch_strategy: Option<String>,
    pub downstream_grain: Option<String>,
    pub input_grains: Vec<Option<String>>,
}

/// A plan section a summary counts and the rows endpoint pages, in the order the plan reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlanPageSection {
    /// Models that couldn't be planned, with their errors.
    PlanningError,
    /// Models an explicit cursor window skips, as it ends before their `cursor_start`.
    WindowBeforeCursorStart,
    /// Built models the plan rebuilds in full.
    FullRefresh,
    QueryChanged,
    FunctionChanged,
    ConfigChanged,
    SchemaChanged,
    /// Built models with no recorded build.
    FirstRun,
    Renamed,
    /// Built models with no change of their own.
    RoutineModels,
    Migrations,
    ColumnMigrations,
    TableTypeConversions,
    Retention,
    LoaderReloads,
    LoaderLoads,
    SourceReloads,
    SourceLoads,
    /// Functions whose definition changed.
    ChangedFunctions,
    /// Functions without a change.
    Functions,
    /// Seeds the run loads for the first time.
    FirstRunSeeds,
    /// Seeds whose definition or settings changed.
    ChangedSeeds,
    /// Seeds the run loads or reports on without a change of their own.
    UnchangedSeeds,
    PythonNodes,
    /// `--changed` nodes held back for a job's own run.
    HeldBack,
    /// `--changed` nodes that run only when something they read changed.
    Unchanged,
    Warnings,
}

/// One page of a plan section's rows matching a name filter, with what its models need beside them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanPage {
    /// Rows matching the filter; the section's size without one.
    pub total: usize,
    /// Rows in the section, filter or not.
    pub section_total: usize,
    pub rows: Vec<serde_json::Value>,
    pub runs: Vec<serde_json::Value>,
    pub warnings: Vec<serde_json::Value>,
    pub cursor_window_starts: serde_json::Map<String, serde_json::Value>,
}

/// A unified diff's changed lines, and whether they differ only in whitespace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiffStats {
    pub added: usize,
    pub removed: usize,
    /// The removed and added lines read the same with every whitespace character taken out.
    pub whitespace_only: bool,
}

/// What a plan will run, by kind, for its one-line summary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanSummaryCounts {
    pub full_refresh: bool,
    /// Models, seeds and functions.
    pub selected: usize,
    pub sources: usize,
    pub loads: usize,
    pub reloads_sources: bool,
    pub python: usize,
    pub audits: usize,
    pub audits_not_run: usize,
}
