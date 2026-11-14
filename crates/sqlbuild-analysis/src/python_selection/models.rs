//! Python node graphs, the SQL side they select against, and selection results.

use std::collections::HashMap;

use crate::assembly::project::types::ObjectKey;
use crate::graph::models::ProjectGraph;

/// One executable Python node as selectors see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonSelectionNode {
    pub name: String,
    /// `task`, `asset`, `loader` or `check`.
    pub kind: String,
    /// The SQL resource keys the node declares, in authored order.
    pub sql_deps: Vec<ObjectKey>,
}

/// Python's `PythonNodeGraph` indexes, each in Python's dict order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PythonSelectionGraph {
    pub nodes: Vec<PythonSelectionNode>,
    pub upstream: Vec<(String, Vec<String>)>,
    pub downstream: Vec<(String, Vec<String>)>,
    pub tags: Vec<(String, Vec<String>)>,
    /// Node name to the folder of its file relative to the project.
    pub paths: Vec<(String, String)>,
}

/// The SQL graph Python nodes are selected with, and each source's loader in project order.
#[derive(Clone, Copy, Debug)]
pub struct SqlSelectionGraph<'graph> {
    pub graph: &'graph ProjectGraph,
    /// `(source name, loader)` for every compiled source.
    pub sources: &'graph [(String, Option<String>)],
}

/// The SQL keys and runnable Python node names a run selects, both sorted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunSelection {
    pub sql_keys: Vec<ObjectKey>,
    pub python_node_names: Vec<String>,
}

/// One discovered Python node as the graph inventory indexes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryNode {
    pub name: String,
    pub tags: Vec<String>,
    /// The node's file relative to the project, in POSIX form.
    pub relative_path: String,
    /// The Python nodes it depends on, in authored order.
    pub upstream: Vec<String>,
}

/// One Python node as a published snapshot lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedPythonNode {
    pub name: String,
    /// `task`, `asset`, `loader` or `check`.
    pub kind: String,
    pub tags: Vec<String>,
    /// The folder of the node's file relative to the project, in POSIX form.
    pub folder: String,
    /// The Python nodes and SQL resources it depends on, as published keys.
    pub deps: Vec<ObjectKey>,
}

/// Python's `PythonNodeGraph` edges and indexes, each in Python's dict order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InventoryIndexes {
    /// `(upstream, downstream)` dependency edges.
    pub edges: Vec<(String, String)>,
    pub upstream: Vec<(String, Vec<String>)>,
    pub downstream: Vec<(String, Vec<String>)>,
    pub tags: Vec<(String, Vec<String>)>,
    pub paths: Vec<(String, String)>,
}

/// One selected resource: a SQL key or a Python node name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Atom {
    Sql(ObjectKey),
    Python(String),
}

/// Python's `nodes_by_name` and edge and tag dicts over a Python node graph, last entry winning.
#[derive(Clone, Debug)]
pub(crate) struct NodeIndex<'graph> {
    pub(crate) graph: &'graph PythonSelectionGraph,
    pub(crate) by_name: HashMap<&'graph str, &'graph PythonSelectionNode>,
    pub(crate) upstream: HashMap<&'graph str, &'graph [String]>,
    pub(crate) downstream: HashMap<&'graph str, &'graph [String]>,
    pub(crate) tags: HashMap<&'graph str, &'graph [String]>,
}

/// Both graphs one unified selection reads, with the SQL selector names indexed.
#[derive(Clone, Debug)]
pub(crate) struct SelectionContext<'graph> {
    pub(crate) sql: SqlSelectionGraph<'graph>,
    pub(crate) nodes: NodeIndex<'graph>,
    pub(crate) sql_names: HashMap<&'graph str, &'graph ObjectKey>,
}
