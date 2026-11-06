//! Python's lineage edges, tag, path and name indexes over one project's resources.

use std::collections::{HashMap, HashSet};

use crate::assembly::project::types::ObjectKey;
use crate::graph::constants::{MODEL_ROOT, MODEL_ROOT_PREFIX, SQL_TEST_RESOURCE};
use crate::graph::main::downstream_edges::downstream_edges;
use crate::graph::models::{GraphIndexes, GraphResource, OrderedEdges, ProjectGraph};

type Edges = Vec<(ObjectKey, Vec<ObjectKey>)>;

impl OrderedEdges {
    /// The list stored for `key`, appended empty in insertion order when the key is new.
    pub(crate) fn list(&mut self, key: &ObjectKey) -> &mut Vec<ObjectKey> {
        let position: usize = match self.index.get(key) {
            Some(&position) => position,
            None => {
                self.entries.push((key.clone(), Vec::new()));
                self.index.insert(key.clone(), self.entries.len() - 1);
                self.entries.len() - 1
            }
        };
        &mut self.entries[position].1
    }

    /// The dependencies already stored for `key`, without inserting a missing target.
    pub(crate) fn get(&self, key: &ObjectKey) -> Option<&Vec<ObjectKey>> {
        self.index
            .get(key)
            .map(|&position| &self.entries[position].1)
    }
}

/// Tag to keys in first-tagged order, each key once, as Python's tag sets hold them.
#[derive(Default)]
struct OrderedTags {
    entries: Vec<(String, Vec<ObjectKey>)>,
    index: HashMap<String, (usize, HashSet<ObjectKey>)>,
}

impl OrderedTags {
    fn add(&mut self, tag: &str, key: &ObjectKey) {
        let (position, seen) = self.index.entry(tag.to_owned()).or_insert_with(|| {
            self.entries.push((tag.to_owned(), Vec::new()));
            (self.entries.len() - 1, HashSet::new())
        });
        if seen.insert(key.clone()) {
            self.entries[*position].1.push(key.clone());
        }
    }
}

impl ProjectGraph {
    /// Point `key`'s name at it; a later resource with the same name replaces the key in place.
    fn set_name(&mut self, name: &str, key: &ObjectKey) {
        match self.name_index.get(name) {
            Some(&position) => self.names[position].1 = key.clone(),
            None => {
                self.name_index.insert(name.to_owned(), self.names.len());
                self.names.push((name.to_owned(), key.clone()));
            }
        }
    }
}

pub(crate) fn build(resources: &[GraphResource]) -> ProjectGraph {
    let mut upstream: OrderedEdges = OrderedEdges::default();
    for resource in resources
        .iter()
        .filter(|resource| resource.key.0 != SQL_TEST_RESOURCE)
    {
        *upstream.list(&resource.key) = lineage_deps(&resource.deps);
    }
    let downstream: Edges = downstream_edges(&upstream.entries);
    let mut tags: OrderedTags = OrderedTags::default();
    let mut graph = ProjectGraph {
        downstream_index: positions(&downstream),
        downstream,
        upstream_index: upstream.index,
        upstream: upstream.entries,
        ..ProjectGraph::default()
    };
    for resource in resources {
        for tag in &resource.tags {
            tags.add(tag, &resource.key);
        }
        if let Some(folder) = &resource.folder {
            graph
                .paths
                .push((resource.key.clone(), below_model_root(folder)));
        }
        graph.set_name(&resource.key.1, &resource.key);
    }
    graph.tags = tags.entries;
    graph
}

/// A graph over indexes a caller already built, such as the planner's.
pub(crate) fn from_indexes(indexes: GraphIndexes) -> ProjectGraph {
    let GraphIndexes {
        names,
        upstream,
        downstream,
        tags,
        paths,
    } = indexes;
    let mut graph = ProjectGraph {
        upstream_index: positions(&upstream),
        downstream_index: positions(&downstream),
        upstream,
        downstream,
        tags,
        paths,
        ..ProjectGraph::default()
    };
    for (name, key) in names {
        graph.set_name(&name, &key);
    }
    graph
}

fn lineage_deps(deps: &[ObjectKey]) -> Vec<ObjectKey> {
    deps.iter()
        .filter(|dep| dep.0 != SQL_TEST_RESOURCE)
        .cloned()
        .collect()
}

fn positions(entries: &[(ObjectKey, Vec<ObjectKey>)]) -> HashMap<ObjectKey, usize> {
    entries
        .iter()
        .enumerate()
        .map(|(position, (key, _))| (key.clone(), position))
        .collect()
}

/// A model directory with forward slashes and the leading `models/` removed.
fn below_model_root(folder: &str) -> String {
    let folder: String = folder.replace('\\', "/");
    if folder == MODEL_ROOT {
        return String::new();
    }
    folder
        .strip_prefix(MODEL_ROOT_PREFIX)
        .map_or_else(|| folder.clone(), str::to_owned)
}
