//! The review plan, the evidence a review collects, and the report it renders.

use std::collections::BTreeMap;

/// Which downstream nodes of the changed set a review builds and compares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DownstreamPolicy {
    /// Every downstream node on a path to a critical model.
    #[default]
    Critical,
    /// Every downstream node.
    All,
    /// No downstream node.
    None,
}

/// One project node: a model or seed a review can build, or a Python node it cannot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewNode {
    pub name: String,
    /// `model` or `seed`, or the Python node kind: `loader`, `asset` or `check`.
    pub kind: String,
    pub critical: bool,
    pub materialized: String,
    pub unique_key: Vec<String>,
    pub has_cursor: bool,
    /// Whether the node is a Python node, which a review never runs.
    pub python: bool,
    /// Whether the node declares pre- or post-hooks.
    pub has_hooks: bool,
}

/// The project graph a review plans over: models, seeds and Python nodes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewGraph {
    pub nodes: BTreeMap<String, ReviewNode>,
    /// Direct downstream nodes by node name.
    pub downstream: BTreeMap<String, Vec<String>>,
    /// Every node name in dependency order.
    pub order: Vec<String>,
}

/// A changed buildable node and why it counts as changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChangedNode {
    pub name: String,
    pub kind: String,
    pub reasons: Vec<String>,
    /// Whether the node is new since the base revision.
    pub added: bool,
}

/// One `--select` token and the node names it resolved to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectedToken {
    pub token: String,
    pub names: Vec<String>,
}

/// A changed or downstream node the review does not build, and why.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NotValidated {
    pub name: String,
    pub reason: String,
}

/// A node built only because a validated node reads it, and the first such node.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AddedNode {
    pub name: String,
    pub required_by: String,
}

/// What a review builds and compares, and what it leaves out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewPlan {
    pub policy: DownstreamPolicy,
    pub changed: Vec<ChangedNode>,
    /// Nodes to build, in dependency order.
    pub validated: Vec<String>,
    /// Changed or downstream nodes left out, in dependency order.
    pub not_validated: Vec<NotValidated>,
    /// Critical nodes the changed set reaches.
    pub critical: Vec<String>,
    /// Nodes outside the selection or policy built because a validated node depends on them.
    pub added: Vec<AddedNode>,
    /// Validated nodes whose hooks run against the review schema.
    pub hooks: Vec<String>,
}

/// One named key or cell value shown as an example.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewExample {
    pub key: String,
    pub target: Option<String>,
    pub branch: Option<String>,
}

/// A difference between the branch and the target, or a failed test or audit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewFinding {
    pub node: String,
    /// `values`, `schema`, `row_count`, `keys`, `test` or `audit`.
    pub kind: String,
    pub metric: String,
    pub column: Option<String>,
    pub target_value: Option<String>,
    pub branch_value: Option<String>,
    pub limit: String,
    pub examples: Vec<ReviewExample>,
}

/// A check the review could not complete for one node.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IncompleteCheck {
    pub node: String,
    pub check: String,
    pub reason: String,
}

/// One column-level difference in a node's comparison evidence.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColumnEvidence {
    pub column: String,
    pub metric: String,
    pub target: String,
    pub branch: String,
    pub limit: String,
}

/// How one node compared with the target.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeComparison {
    /// `full`, `bounded`, `schema_only` or `none`.
    pub mode: String,
    pub target_relation: Option<String>,
    pub branch_relation: Option<String>,
    pub target_rows: Option<i64>,
    pub branch_rows: Option<i64>,
    pub compared_rows: Option<i64>,
    pub unique_key: Vec<String>,
    pub added_columns: Vec<String>,
    pub removed_columns: Vec<String>,
    /// `(column, target type, branch type)`.
    pub type_changes: Vec<(String, String, String)>,
    pub branch_only_keys: i64,
    pub target_only_keys: i64,
    pub columns: Vec<ColumnEvidence>,
    /// Columns compared without a difference.
    pub unchanged_columns: usize,
    /// Whether every check of this node ran to completion.
    pub complete: bool,
}

/// How a changed incremental model started in the review schema.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IncrementalStart {
    pub node: String,
    /// `clone` or `full_refresh`.
    pub method: String,
    pub reason: String,
}

/// Everything one review learned about one node.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeReport {
    pub name: String,
    pub kind: String,
    pub critical: bool,
    pub changed: bool,
    /// `built`, `ok`, `finding`, `failed` or `not_validated`.
    pub status: String,
    pub reason: Option<String>,
    pub build_status: Option<String>,
    pub comparison: Option<NodeComparison>,
    pub affected_columns: Vec<String>,
}

/// Unit test and audit counts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationCounts {
    pub selected: usize,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<String>,
}

/// The review schema and what happened to it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewSchemaReport {
    pub database: Option<String>,
    pub name: String,
    pub created: bool,
    pub dropped: bool,
    pub error: Option<String>,
}

/// The code, message and help of the error that ended a review early.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewFailure {
    pub code: String,
    pub message: String,
    pub help: Option<String>,
}

/// The git and target baselines of one review.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewBaseline {
    pub since_ref: String,
    pub merge_base: String,
    pub head: String,
    pub against: String,
    pub target: Option<String>,
}

/// Everything one `sqb review` run reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReviewReport {
    pub version: String,
    pub baseline: ReviewBaseline,
    pub dry_run: bool,
    pub duration_seconds: f64,
    pub plan: ReviewPlan,
    /// The JSON of the definition changes since the base revision.
    pub changes_json: Option<serde_json::Value>,
    /// Non-buildable changed nodes, such as macros and sources, as `(kind, name, reason)`.
    pub changed_declarations: Vec<(String, String, String)>,
    pub nodes: Vec<NodeReport>,
    /// `(upstream, downstream)` edges between the changed nodes and their downstream.
    pub impact_edges: Vec<(String, String)>,
    pub findings: Vec<ReviewFinding>,
    pub incomplete: Vec<IncompleteCheck>,
    pub incremental_starts: Vec<IncrementalStart>,
    /// Upstream relations read from the target through deferral.
    pub deferred: Vec<String>,
    pub tests: ValidationCounts,
    pub audits: ValidationCounts,
    pub schema: Option<ReviewSchemaReport>,
    pub invocation_id: Option<String>,
    pub error: Option<ReviewFailure>,
}
