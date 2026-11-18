//! One set dependency's members and end nodes, the edges they add, and the cycles those close.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::graph::errors::SelectorError;
use crate::graph::models::JobSelection;
use crate::node_set_dependencies::_helpers::combined::Combined;
use crate::node_set_dependencies::_helpers::labels::{
    excluding, listed, node_label, node_name, set_label,
};
use crate::node_set_dependencies::constants::{
    DEPENDABLE_PYTHON_KINDS, DEPENDABLE_SQL_KINDS, JOB_SET, LOADER_KIND, LOCATION_SEPARATOR,
    NODE_SET_CODE, TAG_SET,
};
use crate::node_set_dependencies::models::{
    DependantNode, GraphNode, NodeSetDependency, ResolvedNode,
};
use crate::python_selection::main::resolve_python_sql_nodes::resolve_python_sql_nodes;
use crate::python_selection::models::{PythonSelectionGraph, RunSelection, SqlSelectionGraph};

pub(crate) fn failure(message: String, help: Option<String>) -> SelectorError {
    SelectorError {
        code: NODE_SET_CODE,
        message,
        help,
    }
}

/// Each Python node's kind, by name.
pub(crate) fn python_kinds(python: &PythonSelectionGraph) -> HashMap<String, String> {
    python
        .nodes
        .iter()
        .map(|node| (node.name.clone(), node.kind.clone()))
        .collect()
}

/// The dependable nodes `dependency` names, less its `exclude`.
pub(crate) fn members(
    (sql, python): (SqlSelectionGraph<'_>, &PythonSelectionGraph),
    (dependant, dependency): (&DependantNode, &NodeSetDependency),
    catalog: &BTreeMap<String, JobSelection>,
) -> Result<BTreeSet<GraphNode>, SelectorError> {
    let owner: String = format!("{} '{}'", dependant.kind, dependant.name);
    let set: String = set_label(dependency);
    if dependant.kind == LOADER_KIND {
        return Err(failure(
            format!("{owner} depends on {set}; loaders cannot depend on a job or a tag"),
            Some("name the loaders, tasks or assets it depends on one by one".to_owned()),
        ));
    }
    if dependency.name.contains(LOCATION_SEPARATOR) {
        return Err(failure(
            format!("{owner} depends on {set}, a job of another code location"),
            Some("depending on another location's jobs is not supported yet".to_owned()),
        ));
    }
    if dependency.kind == JOB_SET && !catalog.contains_key(&dependency.name) {
        return Err(failure(
            format!("{owner} depends on {set}, which the project does not declare"),
            Some(
                "name a job or job group from orchestration/jobs or orchestration/job_groups"
                    .to_owned(),
            ),
        ));
    }
    let tagged: bool = sql
        .graph
        .tags
        .iter()
        .any(|(tag, _)| *tag == dependency.name)
        || python.tags.iter().any(|(tag, _)| *tag == dependency.name);
    if dependency.kind == TAG_SET && !tagged {
        return Err(failure(
            format!("{owner} depends on {set}, which no node carries"),
            None,
        ));
    }
    let exclude: Vec<String> = dependency
        .exclude
        .as_deref()
        .map(|exclude| exclude.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default();
    let selected: RunSelection = resolve_python_sql_nodes(
        sql,
        python,
        &[format!("{}:{}", dependency.kind, dependency.name)],
        &exclude,
    )
    .map_err(|error| {
        failure(
            format!("{owner} depends on {set}: {}", error.message),
            error.help,
        )
    })?;
    let kinds: HashMap<String, String> = python_kinds(python);
    let found: BTreeSet<GraphNode> = selected
        .sql_keys
        .into_iter()
        .filter(|(kind, _)| DEPENDABLE_SQL_KINDS.contains(&kind.as_str()))
        .map(GraphNode::Sql)
        .chain(
            selected
                .python_node_names
                .into_iter()
                .filter(|name| {
                    kinds
                        .get(name)
                        .is_some_and(|kind| DEPENDABLE_PYTHON_KINDS.contains(&kind.as_str()))
                })
                .map(GraphNode::Python),
        )
        .collect();
    if found.is_empty() {
        let after: &str = if exclude.is_empty() {
            ""
        } else {
            " after its exclude"
        };
        return Err(failure(
            format!(
                "{owner} depends on {set}, which leaves no model, seed, source, loader, task or \
                 asset{after}"
            ),
            None,
        ));
    }
    Ok(found)
}

/// The members nothing else in the set reads, directly or through other nodes.
pub(crate) fn end_nodes(combined: &Combined, members: &BTreeSet<GraphNode>) -> Vec<GraphNode> {
    let read: HashSet<GraphNode> = combined.reach_upstream(members, |_| true);
    members
        .iter()
        .filter(|member| !read.contains(*member))
        .cloned()
        .collect()
}

/// `python` with each resolved set's end nodes added to its node's dependencies.
pub(crate) fn with_edges(
    python: &PythonSelectionGraph,
    resolved: &[ResolvedNode],
) -> PythonSelectionGraph {
    let mut graph: PythonSelectionGraph = python.clone();
    for node in resolved {
        for end in node.sets.iter().flat_map(|set| &set.ends) {
            match end {
                GraphNode::Sql(key) => {
                    if let Some(owner) = graph.nodes.iter_mut().find(|item| item.name == node.name)
                        && !owner.sql_deps.contains(key)
                    {
                        owner.sql_deps.push(key.clone());
                    }
                }
                GraphNode::Python(upstream) => {
                    graph.upstream =
                        with_edge(std::mem::take(&mut graph.upstream), &node.name, upstream);
                    graph.downstream =
                        with_edge(std::mem::take(&mut graph.downstream), upstream, &node.name);
                }
            }
        }
    }
    graph
}

/// `rows` with `other` listed against `name` once.
fn with_edge(
    mut rows: Vec<(String, Vec<String>)>,
    name: &str,
    other: &str,
) -> Vec<(String, Vec<String>)> {
    match rows.iter_mut().find(|(row, _)| row == name) {
        Some((_, names)) if names.iter().any(|existing| existing == other) => {}
        Some((_, names)) => names.push(other.to_owned()),
        None => rows.push((name.to_owned(), vec![other.to_owned()])),
    }
    rows
}

/// The `readers` no other reader stands between and `dependant`.
fn nearest_readers<'a>(
    explicit: &Combined,
    readers: &[&'a GraphNode],
    dependant: &GraphNode,
) -> Vec<&'a GraphNode> {
    let mut nearest: Vec<&GraphNode> = Vec::new();
    for reader in readers {
        let reached: HashSet<GraphNode> =
            explicit.reach_upstream([*reader], |node| !readers.contains(&node));
        if reached.contains(dependant) {
            nearest.push(reader);
        }
    }
    nearest
}

/// One resolved set with the members it was reduced from, for cycle errors.
pub(crate) struct SetMembers<'a> {
    pub(crate) dependant: &'a DependantNode,
    pub(crate) dependency: &'a NodeSetDependency,
    pub(crate) members: BTreeSet<GraphNode>,
    pub(crate) ends: Vec<GraphNode>,
}

/// The set whose added edge is the cycle step `pair`, a dependant reading one of its end nodes.
fn closing_set<'s, 'a>(
    explicit: &Combined,
    sets: &'s [SetMembers<'a>],
    pair: &[GraphNode],
) -> Option<&'s SetMembers<'a>> {
    let GraphNode::Python(name) = &pair[0] else {
        return None;
    };
    let named: bool = explicit.upstream(&pair[0]).contains(&pair[1]);
    sets.iter()
        .find(|set| set.dependant.name == *name && set.ends.contains(&pair[1]) && !named)
}

/// The error for a dependency cycle a set dependency closes; `None` when every cycle predates them.
pub(crate) fn set_cycle(
    (explicit, current): (&Combined, &Combined),
    sets: &[SetMembers<'_>],
    kinds: &HashMap<String, String>,
) -> Option<SelectorError> {
    let cycle: Vec<GraphNode> = current.python_cycle()?;
    let (index, set): (usize, &SetMembers<'_>) = cycle
        .windows(2)
        .enumerate()
        .find_map(|(index, pair)| Some((index, closing_set(explicit, sets, pair)?)))?;
    let dependant: GraphNode = cycle[index].clone();
    let owner: String = node_label(&dependant, kinds);
    let label: String = set_label(set.dependency);
    if set.members.contains(&dependant) {
        return Some(failure(
            format!("{owner} depends on {label}, which includes {owner} itself"),
            Some(format!(
                "leave it out of the dependency: {}",
                excluding(set.dependency, &[node_name(&dependant)])
            )),
        ));
    }
    let downstream: HashSet<GraphNode> = explicit.reach_downstream(&dependant);
    let readers: Vec<&GraphNode> = set
        .members
        .iter()
        .filter(|member| downstream.contains(*member))
        .collect();
    let nearest: Vec<&GraphNode> = nearest_readers(explicit, &readers, &dependant);
    let closer: &GraphNode = nearest.first().copied().or(readers.first().copied())?;
    let path: Vec<String> = explicit
        .path_upstream(closer, &dependant)
        .iter()
        .map(|node| node_label(node, kinds))
        .collect();
    let nearest_labels: Vec<String> = nearest.iter().map(|node| node_label(node, kinds)).collect();
    let reader_names: Vec<&str> = readers.iter().map(|node| node_name(node)).collect();
    let owner_of_set: String = match set.dependency.kind.as_str() {
        JOB_SET => format!("{label}'s"),
        _ => format!("the {label} node"),
    };
    let remedy: &str = match set.dependency.kind.as_str() {
        JOB_SET => "move it to another job",
        _ => "remove the tag from it",
    };
    Some(failure(
        format!(
            "{owner} depends on {label}, but {owner_of_set} {} reads {owner}'s output ({}), \
             a dependency cycle",
            listed(&nearest_labels),
            path.join(" <- ")
        ),
        Some(format!(
            "{remedy}, or leave every node of the set that reads {owner} out of the dependency: {}",
            excluding(set.dependency, &reader_names)
        )),
    ))
}
