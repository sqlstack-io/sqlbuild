//! `resolve_node_set_dependencies`: each `job(...)` and `tag(...)` dependency as edges to end nodes.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::graph::errors::SelectorError;
use crate::graph::models::{JobSelection, ProjectGraph};
use crate::node_set_dependencies::_helpers::combined::Combined;
use crate::node_set_dependencies::_helpers::group_loops::group_loop;
use crate::node_set_dependencies::_helpers::sets::{
    SetMembers, end_nodes, members, python_kinds, set_cycle, with_edges,
};
use crate::node_set_dependencies::models::{DependantNode, GraphNode, ResolvedNode, ResolvedSet};
use crate::python_selection::main::attach_python_jobs::attach_python_jobs;
use crate::python_selection::models::{PythonSelectionGraph, SqlSelectionGraph};

/// Each dependant's sets reduced to end nodes, resolved again over added edges until none change.
pub fn resolve_node_set_dependencies(
    sql: SqlSelectionGraph<'_>,
    python: &PythonSelectionGraph,
    dependants: &[DependantNode],
    catalog: &BTreeMap<String, JobSelection>,
) -> Result<Vec<ResolvedNode>, SelectorError> {
    if dependants.is_empty() {
        return Ok(Vec::new());
    }
    let jobs: ProjectGraph = attach_python_jobs(sql, python, catalog.clone());
    let with_jobs: SqlSelectionGraph<'_> = SqlSelectionGraph {
        graph: &jobs,
        ..sql
    };
    let kinds: HashMap<String, String> = python_kinds(python);
    let explicit: Combined = Combined::new(sql, python);
    let mut current: PythonSelectionGraph = python.clone();
    let mut sets: Vec<SetMembers<'_>> = Vec::new();
    for _ in 0..=dependants.len() {
        let combined: Combined = Combined::new(sql, &current);
        sets = Vec::new();
        for dependant in dependants {
            for dependency in &dependant.sets {
                let found: BTreeSet<GraphNode> =
                    members((with_jobs, &current), (dependant, dependency), catalog)?;
                let ends: Vec<GraphNode> = end_nodes(&combined, &found);
                sets.push(SetMembers {
                    dependant,
                    dependency,
                    members: found,
                    ends,
                });
            }
        }
        let next: PythonSelectionGraph = with_edges(python, &resolved(&sets));
        if let Some(error) = set_cycle((&explicit, &Combined::new(sql, &next)), &sets, &kinds) {
            return Err(error);
        }
        if next == current {
            break;
        }
        current = next;
    }
    let resolved: Vec<ResolvedNode> = resolved(&sets);
    let current: Combined = Combined::new(sql, &current);
    match group_loop(&jobs, catalog, (&explicit, &current), (&sets, &kinds)) {
        Some(error) => Err(error),
        None => Ok(resolved),
    }
}

/// Each dependant's resolved sets, in declaration order.
fn resolved(sets: &[SetMembers<'_>]) -> Vec<ResolvedNode> {
    let mut nodes: Vec<ResolvedNode> = Vec::new();
    for set in sets {
        let resolved: ResolvedSet = ResolvedSet {
            dependency: set.dependency.clone(),
            ends: set.ends.clone(),
        };
        match nodes
            .iter_mut()
            .find(|node| node.name == set.dependant.name)
        {
            Some(node) => node.sets.push(resolved),
            None => nodes.push(ResolvedNode {
                name: set.dependant.name.clone(),
                sets: vec![resolved],
            }),
        }
    }
    nodes
}
