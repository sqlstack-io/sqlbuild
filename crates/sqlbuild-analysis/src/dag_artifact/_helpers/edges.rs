//! `DagEdge` values: lineage, loader dependency and Python node edges, sorted.

use std::collections::HashMap;

use crate::dag_artifact::_helpers::nodes::{
    loader_or_source_id, node_id, python_node_id, sources_by_loader,
};
use crate::dag_artifact::_helpers::values::{object, text};
use crate::dag_artifact::constants::LOADER_KIND;
use crate::dag_artifact::models::{DagProjectInput, DagPythonGraphInput};
use crate::dag_artifact::types::DagValue;
use crate::graph::models::ProjectGraph;

/// Each Python node name's kind, as `nodes_by_name` maps it.
pub(crate) fn python_kinds(python: &DagPythonGraphInput) -> HashMap<&str, &str> {
    python
        .kinds_by_name
        .iter()
        .map(|(name, kind)| (name.as_str(), kind.as_str()))
        .collect()
}

/// Python dependency and SQL read edges; a loader a source declares is drawn as that source.
fn python_edges(
    python: &DagPythonGraphInput,
    sources: &HashMap<&str, &str>,
) -> Vec<(String, String)> {
    let kinds: HashMap<&str, &str> = python_kinds(python);
    let id = |name: &str| {
        let kind: &str = kinds[name];
        if kind == LOADER_KIND {
            loader_or_source_id(name, sources)
        } else {
            python_node_id(kind, name)
        }
    };
    let mut edges: Vec<(String, String)> = python
        .dependency_edges
        .iter()
        .map(|(upstream, downstream)| (id(upstream), id(downstream)))
        .collect();
    for node in &python.nodes {
        for dependency in &node.sql_deps {
            edges.push((node_id(dependency), id(&node.name)));
        }
    }
    edges
}

/// Every edge, sorted by `(from_id, to_id)`.
pub(crate) fn edges(
    graph: &ProjectGraph,
    project: &DagProjectInput,
    python: Option<&DagPythonGraphInput>,
) -> DagValue {
    let mut edges: Vec<(String, String)> = Vec::new();
    for (key, deps) in &graph.upstream {
        for dep in deps {
            edges.push((node_id(dep), node_id(key)));
        }
    }
    let sources: HashMap<&str, &str> = sources_by_loader(project);
    for loader in &project.loaders {
        for dependency in &loader.dependency_names {
            edges.push((
                loader_or_source_id(dependency, &sources),
                loader_or_source_id(&loader.name, &sources),
            ));
        }
    }
    if let Some(python) = python {
        edges.extend(python_edges(python, &sources));
    }
    edges.sort();
    DagValue::Array(
        edges
            .iter()
            .map(|(from, to)| object(vec![("from_id", text(from)), ("to_id", text(to))]))
            .collect(),
    )
}
