//! Planner identity, pruning and graph-order data.

use std::collections::{HashMap, HashSet};

use crate::assembly::project::types::ObjectKey;

/// A graph node identity: resource type (or fingerprint node type) and name, ordered as Python's `(resource_type, name)`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GraphKey {
    pub kind: String,
    pub name: String,
}

impl From<&ObjectKey> for GraphKey {
    fn from((kind, name): &ObjectKey) -> Self {
        Self {
            kind: kind.clone(),
            name: name.clone(),
        }
    }
}

/// One SQL test woven into its targets' execution dependencies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionTest {
    pub key: ObjectKey,
    pub scope_deps: Vec<ObjectKey>,
    pub table_function: bool,
}

/// One node of an identity graph: its upstream keys in order and its local hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityNode {
    pub upstream_keys: Vec<GraphKey>,
    pub local_hash: Option<String>,
}

/// An identity graph: nodes by key and the order nodes are resolved in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdentityGraph {
    pub nodes: HashMap<GraphKey, IdentityNode>,
    pub execution_order: Vec<GraphKey>,
}

/// A function definition's fingerprint fields, already rendered as text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FunctionFingerprintFields {
    pub language: String,
    pub arguments: Vec<(String, String)>,
    pub returns: String,
    pub return_columns: Vec<(String, String)>,
    pub runtime_version: String,
    pub entry_point: String,
    pub packages: Vec<String>,
    pub body_sql: String,
}

/// Names that changed or run this time, per resource kind, for stale-selection classification.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StalenessNames {
    pub models: HashSet<String>,
    pub seeds: HashSet<String>,
    pub sources: HashSet<String>,
}

/// Graph inputs of selected-model stale warning classification.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StalenessGraph {
    pub upstream_deps: HashMap<GraphKey, Vec<GraphKey>>,
    pub selected_model_names: Vec<String>,
    pub run: StalenessNames,
    pub changed: StalenessNames,
}

/// A selected model that builds on changed upstreams outside the run set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaleWarning {
    pub model_name: String,
    pub trigger_names: Vec<String>,
}

/// What satisfies an upstream dependency of a selected key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AvailableUpstreams {
    pub satisfied_keys: HashSet<GraphKey>,
    pub existing_relations: HashSet<String>,
    pub deferred_relations: Option<HashSet<String>>,
}

/// One upstream dependency missing from both scope and warehouse, with its dependents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingUpstream {
    pub key: GraphKey,
    pub required_by: Vec<GraphKey>,
}
