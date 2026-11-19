//! The plain project facts the explicit-reference checks read, and what they report.

use serde::{Deserialize, Serialize};

use crate::explicit_references::types::ResourceKey;

/// Everything the checks read, collected from the compiled project and user Python by Python.
#[derive(Debug, Clone, Deserialize)]
pub struct ReferenceRequest {
    /// `[references] enforce_explicit`: whether Python SQL is checked at all.
    pub enforce_explicit: bool,
    pub dialect: Option<String>,
    /// `explicit_references_help(allowed="hard-coded relation names")`.
    pub explicit_help: String,
    /// The text Python substitutes for f-string values in literal SQL.
    pub placeholder: String,
    /// Models, sources, seeds and functions in project order, with their dependencies.
    pub resources: Vec<GraphResource>,
    pub models: Vec<ModelReferences>,
    pub sources: Vec<SourceRelation>,
    pub seeds: Vec<NamedRelation>,
    pub audits: Vec<AuditGate>,
    /// `project.hook_functions` in order.
    pub hooks: Vec<PythonHook>,
    /// Python SQL owners: assets, checks, loaders, then hooks.
    pub owners: Vec<SqlOwner>,
}

/// One resource's key and dependencies.
#[derive(Debug, Clone, Deserialize)]
pub struct GraphResource {
    pub key: ResourceKey,
    pub deps: Vec<ResourceKey>,
}

/// A relation name with the qualifiers written or configured.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Relation {
    pub name: String,
    pub schema: Option<String>,
    pub database: Option<String>,
}

/// One model's destination, hooks and old name.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelReferences {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub destination: Relation,
    /// `model_python_hook_names(model)`.
    pub python_hooks: Vec<String>,
    pub sql_hooks: Vec<SqlHook>,
    /// The `migrate_from` text where it is a string and old-name views are retained.
    pub migrate_from: Option<String>,
}

/// The facts execution ordering reads: models with hooks, audits and Python hooks.
#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionGateRequest {
    /// Models in project order with their dependencies.
    pub resources: Vec<GraphResource>,
    pub models: Vec<ModelReferences>,
    pub audits: Vec<AuditGate>,
    pub hooks: Vec<PythonHook>,
}

/// One ordering edge: `gated` waits for `read`, which audit `audit_name` on `target` reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GateEdge {
    pub audit_name: String,
    pub target: ResourceKey,
    pub gated: ResourceKey,
    pub read: ResourceKey,
}

/// How diagnostics name a hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookType {
    Python,
    Sql,
    InlineSql,
}

/// One ordering edge: model `gated` waits for `read`, which one of its hooks reads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HookEdge {
    pub hook_name: String,
    pub gated: ResourceKey,
    pub read: ResourceKey,
    pub hook_type: HookType,
    pub path: Option<String>,
}

/// Python's attached-audit gate edges and hook-read edges, in its order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionGateEdges {
    pub audit_gates: Vec<GateEdge>,
    pub hook_reads: Vec<HookEdge>,
}

/// One SQL hook entry with declared reads: its hook key, position and identity.
#[derive(Debug, Clone, Deserialize)]
pub struct SqlHook {
    pub hook_key: String,
    pub index: usize,
    pub name: Option<String>,
    pub path: Option<String>,
    pub reads: Vec<ResourceKey>,
}

/// One source: its table or name and namespace, or an expression source.
#[derive(Debug, Clone, Deserialize)]
pub struct SourceRelation {
    pub name: String,
    pub table: Option<String>,
    pub schema: Option<String>,
    pub database: Option<String>,
    pub expression: bool,
}

/// A seed's name and destination.
#[derive(Debug, Clone, Deserialize)]
pub struct NamedRelation {
    pub name: String,
    pub relation: Relation,
}

/// One audit: its file, attached target and scope dependencies.
#[derive(Debug, Clone, Deserialize)]
pub struct AuditGate {
    pub name: String,
    pub path: String,
    pub target: Option<ResourceKey>,
    pub scope_deps: Vec<ResourceKey>,
}

/// One Python hook function and the resources it declares it reads.
#[derive(Debug, Clone, Deserialize)]
pub struct PythonHook {
    pub name: String,
    pub path: String,
    pub reads: Vec<ResourceKey>,
}

/// Python code whose literal SQL is checked, and what it may name itself.
#[derive(Debug, Clone, Deserialize)]
pub struct SqlOwner {
    pub label: String,
    /// `HardCodedRelationOwnerKind`: `node`, `hook` or `loader`.
    pub kind: String,
    /// The owner's position among its project's paths in Python's `Path` order.
    pub path_order: usize,
    pub own_refs: Vec<ResourceKey>,
    /// Source name to the upstream loader function that writes it.
    pub upstream_loaders: Vec<(String, String)>,
    pub literals: Vec<LiteralSql>,
}

/// One literal SQL string passed to `ctx.query` or `ctx.execute_sql`.
#[derive(Debug, Clone, Deserialize)]
pub struct LiteralSql {
    pub method: String,
    pub sql: String,
    pub line: usize,
}

/// The diagnostics in Python's order and literal SQL relations that matched no project relation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReferenceReport {
    pub diagnostics: Vec<ReferenceDiagnostic>,
    pub unmatched: Vec<UnmatchedRelation>,
}

/// One compile error diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReferenceDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub help: String,
    /// A file path as Python passed it, or None.
    pub path: Option<String>,
    /// The owner whose path and line locate a Python-SQL diagnostic.
    pub owner: Option<usize>,
    pub line: Option<usize>,
    pub resource: Option<ResourceKey>,
}

/// A relation literal SQL names that is not a project relation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct UnmatchedRelation {
    pub owner: usize,
    pub line: usize,
    pub method: String,
    pub relation: Relation,
}
