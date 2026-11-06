//! The project graph, the resources it is built from and selector failures.

use std::collections::HashMap;

use crate::assembly::project::types::ObjectKey;

/// Dependency lists indexed without losing project insertion order.
#[derive(Debug, Default)]
pub(crate) struct OrderedEdges {
    pub(crate) entries: Vec<(ObjectKey, Vec<ObjectKey>)>,
    pub(crate) index: HashMap<ObjectKey, usize>,
}

/// One graph resource in project order: its key, upstream keys, tags and model folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphResource {
    pub key: ObjectKey,
    pub deps: Vec<ObjectKey>,
    pub tags: Vec<String>,
    /// A model's directory relative to the project, `models/` prefix optional; `None` otherwise.
    pub folder: Option<String>,
}

/// Python's `ProjectGraph` indexes, each in Python's dict insertion order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectGraph {
    /// Lineage edges with SQL tests stripped.
    pub upstream: Vec<(ObjectKey, Vec<ObjectKey>)>,
    /// Inverted edges, each list sorted by `(resource type, name)`.
    pub downstream: Vec<(ObjectKey, Vec<ObjectKey>)>,
    /// Tag to keys, keys in first-tagged order.
    pub tags: Vec<(String, Vec<ObjectKey>)>,
    /// Model key to its folder below `models/`.
    pub paths: Vec<(ObjectKey, String)>,
    /// Selector name to key; a later resource with the same name replaces the key in place.
    pub names: Vec<(String, ObjectKey)>,
    pub(crate) upstream_index: HashMap<ObjectKey, usize>,
    pub(crate) downstream_index: HashMap<ObjectKey, usize>,
    pub(crate) name_index: HashMap<String, usize>,
}

/// One parsed selector token: `kind:value` (kind `name` when bare) or `start~end`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedSelector {
    Kind {
        kind: String,
        value: String,
        upstream: bool,
        downstream: bool,
    },
    Path {
        start: String,
        end: String,
        upstream: bool,
        downstream: bool,
    },
}

/// Which resources `expand_required_build_resources` adds around a selected scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildResources {
    pub upstream_functions: bool,
    pub upstream_seeds: bool,
    pub downstream_functions: bool,
}

impl BuildResources {
    /// What `--select`/`--exclude` resolution adds: upstream functions only.
    pub const SELECTION: Self = Self {
        upstream_functions: true,
        upstream_seeds: false,
        downstream_functions: false,
    };
}

/// Python's `ProjectGraph` indexes as handed in, in Python's dict order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphIndexes {
    pub names: Vec<(String, ObjectKey)>,
    pub upstream: Vec<(ObjectKey, Vec<ObjectKey>)>,
    pub downstream: Vec<(ObjectKey, Vec<ObjectKey>)>,
    pub tags: Vec<(String, Vec<ObjectKey>)>,
    pub paths: Vec<(ObjectKey, String)>,
}

/// Which edge direction a graph query follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeDirection {
    Upstream,
    Downstream,
}
