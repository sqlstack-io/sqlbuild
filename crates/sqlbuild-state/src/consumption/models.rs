//! Consumption rows, the per-node facts `--changed` decides from, and its run reasons.

/// A node identity: its resource kind, such as `model` or `seed`, and its name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeKey {
    pub kind: String,
    pub name: String,
}

/// The relation a node writes, as the state table keys it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RelationKey {
    pub database: Option<String>,
    pub schema: String,
    pub name: String,
}

/// One input an execution consumed: its kind, name and the version it read.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConsumedInput {
    pub kind: String,
    pub name: String,
    pub version: String,
}

/// One row to append; the warehouse clock fills `recorded_at`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsumptionAppend {
    pub execution_id: String,
    pub phase: String,
    pub run_id: String,
    pub node: NodeKey,
    pub relation: RelationKey,
    pub version_hash: String,
    pub input_count: i64,
    pub input: Option<ConsumedInput>,
}

/// One row read back, with its warehouse time in microseconds since the epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsumptionRow {
    pub row: ConsumptionAppend,
    pub recorded_at: i64,
}

/// A direct input of a node as this build sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputFact {
    /// A source and its observed `data_version_hash`, or None when unknown.
    Source {
        name: String,
        version: Option<String>,
    },
    /// An upstream model or seed.
    Node(NodeKey),
    /// An input whose version cannot be known, such as a Python node.
    Opaque(String),
}

/// Why the planner already runs a node, whatever its records say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForcedRun {
    /// The planner detected a change of the node itself, named by its change kind.
    PlannerChange(String),
    FullRefresh,
    RelationMissing,
    CursorInterval,
    /// The compiler cannot enumerate what the node reads.
    InputsNotEnumerable,
}

/// One node of the selection or its ancestry, as `--changed` decides from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeFacts {
    pub key: NodeKey,
    pub relation: RelationKey,
    pub version_hash: String,
    pub selected: bool,
    pub forced: Vec<ForcedRun>,
    pub inputs: Vec<InputFact>,
}

/// Why `--changed` runs a node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunReason {
    Forced(ForcedRun),
    NoRecord,
    Unfinished,
    AmbiguousOrder,
    DefinitionChanged,
    InputsChanged,
    InputChanged(String),
    /// An upstream was rebuilt with a definition change this node has not read yet.
    UpstreamDefinitionChanged(String),
    InputUnknown(String),
    UpstreamRuns(NodeKey),
}

/// What a change is, for `[changes] counts`; reasons with no kind always count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChangeKind {
    /// SQL, config, hooks, contract or a called function changed, here or in a rebuilt upstream.
    Definition,
    /// New data: a source's freshness value moved, or an upstream was rebuilt with the same definitions.
    SourceData,
}

/// A job or group that a schedule or sensor starts, with every node a run of it builds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriggeredJob {
    pub name: String,
    pub nodes: std::collections::BTreeSet<NodeKey>,
}

/// How far a change reaches downstream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reach {
    /// Stop at nodes that a triggered job without the change refreshes.
    Job(Vec<TriggeredJob>),
    All,
}

/// What `--changed` counts and how far it reaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedPolicy {
    pub counts: std::collections::BTreeSet<ChangeKind>,
    pub reach: Reach,
}

/// Why a node downstream of a change waits for a later run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeldBack {
    /// These triggered jobs refresh it, and none of them holds the change.
    RefreshedBy(Vec<String>),
    /// It reads this node, which is held back.
    ReadsHeldBack(NodeKey),
}

/// What `--changed` does with one selected node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangedOutcome {
    Runs(RunReason),
    Unchanged,
    HeldBack(HeldBack),
}

/// One execution of a relation rebuilt from its deduplicated rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Execution {
    pub execution_id: String,
    /// The earliest warehouse time of its rows.
    pub started_at: i64,
    /// Whether its `succeeded` rows are present, consistent and complete.
    pub complete: bool,
    pub version_hash: Option<String>,
    /// Input versions of a complete execution, keyed by `(kind, name)`.
    pub inputs: Vec<ConsumedInput>,
}

/// The recent executions of each relation, newest first, as `--changed` reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConsumptionHistory {
    pub(crate) relations: std::collections::HashMap<RelationKey, RelationHistory>,
    /// Older executions read back because a newer build read them, by relation and execution ID.
    pub(crate) referenced: std::collections::HashMap<(RelationKey, String), Execution>,
}

/// One relation's executions, newest first, and whether older ones were left unread.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RelationHistory {
    pub(crate) executions: Vec<Execution>,
    pub(crate) truncated: bool,
}
