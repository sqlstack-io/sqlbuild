//! Planner strategy inputs and decisions.

use crate::planner_changes::models::SchemaFindingRow;

/// How a model is materialized; an unknown `materialized` value is a custom materialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterializationType {
    View,
    Table,
    Incremental,
    Snapshot,
    Seed,
    Custom,
    /// A stored result the warehouse keeps fresh between builds.
    Maintained,
}

/// What the plan does with a model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanAction {
    CreateView,
    CreateTable,
    IncrementalAppend,
    IncrementalDeleteInsert,
    IncrementalMerge,
    Snapshot,
    Skip,
    Custom,
    /// Create or replace the warehouse-maintained object.
    CreateMaintained,
    /// Change its refresh settings in place.
    AlterMaintained,
    /// Leave the object as it is; builds may still refresh it.
    KeepMaintained,
}

/// Why the plan does it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanReason {
    FirstRun,
    Renamed,
    FullRefresh,
    QueryChanged,
    FunctionChanged,
    ConfigChanged,
    SchemaChanged,
    NormalIncremental,
    NoChange,
    Disabled,
}

/// How a model changed since its recorded build, including first runs and renames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanChangeKind {
    FirstRun,
    Renamed,
    QueryChanged,
    FunctionChanged,
    ConfigChanged,
    SchemaChanged,
    NoChange,
}

/// The parts of a model's change result the strategy reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeFacts {
    pub kind: PlanChangeKind,
    pub backfill_full: bool,
    pub backfill_forward_only: bool,
    pub query_changed: bool,
    pub changed_functions: Vec<String>,
    pub config_changed: bool,
    pub schema_findings: Vec<SchemaFindingRow>,
    pub recorded_build_relation_missing: bool,
}

/// The model configuration the plan action depends on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelStrategy {
    pub name: String,
    pub disabled: bool,
    pub materialization: MaterializationType,
    /// The `incremental_strategy` value when it is a string.
    pub incremental_strategy: Option<String>,
}

/// What a schema action does to one column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaActionKind {
    AddColumn,
    DropColumn,
    AlterColumnType,
}

/// One schema change to apply to a target table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaActionRow {
    pub kind: SchemaActionKind,
    pub column_name: String,
    pub column_type: Option<String>,
}

/// How severe a plan warning is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningSeverity {
    Error,
    Warning,
    Info,
}

/// One plan warning about a model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarningRow {
    pub severity: WarningSeverity,
    pub message: String,
    pub code: Option<&'static str>,
}

/// The inputs of a model's plan warnings besides its change facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarningInputs {
    pub model_name: String,
    pub materialization: MaterializationType,
    pub has_schema_actions: bool,
    pub on_schema_change: Option<String>,
    pub type_enforcement: bool,
}

/// A cursor type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorType {
    Timestamp,
    Integer,
}

/// A model's cursor column and the warehouse columns its type is checked against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorTypeCheck {
    pub model_name: String,
    pub cursor_column: Option<String>,
    pub cursor_type: Option<String>,
    pub warehouse_columns: Vec<(String, String)>,
    pub sql_analysis_enabled: bool,
}

/// Where an audit attaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditAttachmentKind {
    Source,
    Seed,
    Model,
    End,
}

/// When an audit runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditRunScope {
    Final,
    DeltaAndFinal,
}

/// The kind and name of one SQL reference.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReferenceIdentity {
    pub kind: String,
    pub name: String,
}

/// The batch-limit facts of an upstream model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProducerFacts {
    pub microbatch: bool,
    pub capped: bool,
}

/// The end a microbatch cap keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapDirection {
    FromStart,
    FromEnd,
}

/// The bound a cap moves: a new end when capping from the start, a new start when capping from the end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CappedBound<T> {
    End(T),
    Start(T),
}

/// Which declaration a cursor input column is checked against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorInputColumnOwner {
    ModelContract,
    SourceContract,
    SourceWarehouse,
}

/// One authored cursor input column and the columns its input is known to expose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorInputColumnCheck {
    pub model_name: String,
    pub owner: CursorInputColumnOwner,
    pub input_name: String,
    pub column: String,
    pub known_columns: Vec<String>,
}

/// The configuration a microbatch model's default lookback is resolved from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LookbackConfig {
    pub lookback: Option<String>,
    pub incremental_strategy: Option<String>,
    pub batch_size: Option<String>,
    pub grain: Option<String>,
}
