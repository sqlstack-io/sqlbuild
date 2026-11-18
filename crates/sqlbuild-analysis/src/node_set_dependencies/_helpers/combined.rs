//! The combined SQL and Python dependency graph a set dependency is reduced and checked over.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use sqlbuild_core::graph_walk::main::find_cycle::find_cycle;

use crate::node_set_dependencies::constants::SOURCE_KIND;
use crate::node_set_dependencies::models::GraphNode;
use crate::python_selection::models::{PythonSelectionGraph, SqlSelectionGraph};

/// Each node's upstream and downstream nodes, a source's loader upstream of it.
pub(crate) struct Combined {
    upstream: HashMap<GraphNode, Vec<GraphNode>>,
    downstream: HashMap<GraphNode, Vec<GraphNode>>,
    python: Vec<GraphNode>,
}

impl Combined {
    pub(crate) fn new(sql: SqlSelectionGraph<'_>, python: &PythonSelectionGraph) -> Self {
        let mut upstream: HashMap<GraphNode, Vec<GraphNode>> = HashMap::new();
        for (key, dependencies) in &sql.graph.upstream {
            upstream
                .entry(GraphNode::Sql(key.clone()))
                .or_default()
                .extend(dependencies.iter().cloned().map(GraphNode::Sql));
        }
        for (source, loader) in sql.sources {
            if let Some(loader) = loader {
                upstream
                    .entry(GraphNode::Sql((SOURCE_KIND.to_owned(), source.clone())))
                    .or_default()
                    .push(GraphNode::Python(loader.clone()));
            }
        }
        for (name, dependencies) in &python.upstream {
            upstream
                .entry(GraphNode::Python(name.clone()))
                .or_default()
                .extend(dependencies.iter().cloned().map(GraphNode::Python));
        }
        for node in &python.nodes {
            upstream
                .entry(GraphNode::Python(node.name.clone()))
                .or_default()
                .extend(node.sql_deps.iter().cloned().map(GraphNode::Sql));
        }
        let mut downstream: HashMap<GraphNode, Vec<GraphNode>> = HashMap::new();
        for (node, dependencies) in &upstream {
            for dependency in dependencies {
                downstream
                    .entry(dependency.clone())
                    .or_default()
                    .push(node.clone());
            }
        }
        Self {
            upstream,
            downstream,
            python: python
                .nodes
                .iter()
                .map(|node| GraphNode::Python(node.name.clone()))
                .collect(),
        }
    }

    pub(crate) fn upstream(&self, node: &GraphNode) -> &[GraphNode] {
        self.upstream.get(node).map_or(&[], Vec::as_slice)
    }

    /// Every node one or more steps upstream of `starts`, walking only nodes `through` allows.
    pub(crate) fn reach_upstream<'a>(
        &self,
        starts: impl IntoIterator<Item = &'a GraphNode>,
        through: impl Fn(&GraphNode) -> bool,
    ) -> HashSet<GraphNode> {
        self.reach(starts, |node| self.upstream(node), through)
    }

    /// Every node one or more steps downstream of `start`.
    pub(crate) fn reach_downstream(&self, start: &GraphNode) -> HashSet<GraphNode> {
        self.reach(
            [start],
            |node| self.downstream.get(node).map_or(&[], Vec::as_slice),
            |_| true,
        )
    }

    fn reach<'a, 'n>(
        &'n self,
        starts: impl IntoIterator<Item = &'a GraphNode>,
        next: impl Fn(&GraphNode) -> &'n [GraphNode],
        through: impl Fn(&GraphNode) -> bool,
    ) -> HashSet<GraphNode> {
        let mut reached: HashSet<GraphNode> = HashSet::new();
        let mut queue: VecDeque<GraphNode> = starts.into_iter().cloned().collect();
        while let Some(node) = queue.pop_front() {
            for neighbour in next(&node) {
                if reached.insert(neighbour.clone()) && through(neighbour) {
                    queue.push_back(neighbour.clone());
                }
            }
        }
        reached
    }

    /// A dependency cycle through a Python node, as the path from it back to itself.
    pub(crate) fn python_cycle(&self) -> Option<Vec<GraphNode>> {
        let path: Vec<GraphNode> = find_cycle(self.python.iter().cloned(), |node| {
            self.upstream(node).to_vec()
        })?;
        let last: &GraphNode = path.last()?;
        let start: usize = path.iter().position(|node| node == last)?;
        Some(path[start..].to_vec())
    }

    /// The shortest path from `from` up to `to`, both included.
    pub(crate) fn path_upstream(&self, from: &GraphNode, to: &GraphNode) -> Vec<GraphNode> {
        let mut parents: HashMap<GraphNode, GraphNode> = HashMap::new();
        let mut queue: VecDeque<GraphNode> = VecDeque::from([from.clone()]);
        let mut seen: BTreeSet<GraphNode> = BTreeSet::from([from.clone()]);
        while let Some(node) = queue.pop_front() {
            if node == *to {
                let mut path: Vec<GraphNode> = vec![node.clone()];
                let mut current: GraphNode = node;
                while let Some(parent) = parents.get(&current) {
                    path.push(parent.clone());
                    current = parent.clone();
                }
                path.reverse();
                return path;
            }
            for neighbour in self.upstream(&node) {
                if seen.insert(neighbour.clone()) {
                    parents.insert(neighbour.clone(), node.clone());
                    queue.push_back(neighbour.clone());
                }
            }
        }
        vec![from.clone(), to.clone()]
    }
}
