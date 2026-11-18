//! A Python node's job and tag dependencies and the end nodes each resolves to.

use crate::assembly::project::types::ObjectKey;

/// One `job(...)` or `tag(...)` dependency as authored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeSetDependency {
    /// `job` or `tag`.
    pub kind: String,
    pub name: String,
    /// Selector tokens removed from the set before it is reduced to its end nodes.
    pub exclude: Option<String>,
}

/// A Python node and the set dependencies it declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DependantNode {
    pub name: String,
    /// The node's kind, such as `task`.
    pub kind: String,
    pub sets: Vec<NodeSetDependency>,
}

/// A node of the combined SQL and Python graph.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GraphNode {
    Sql(ObjectKey),
    Python(String),
}

/// One set dependency and the end nodes it resolved to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSet {
    pub dependency: NodeSetDependency,
    /// The set's dependable nodes nothing else in the set reads, in a stable order.
    pub ends: Vec<GraphNode>,
}

/// A Python node's set dependencies, each resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedNode {
    pub name: String,
    pub sets: Vec<ResolvedSet>,
}
