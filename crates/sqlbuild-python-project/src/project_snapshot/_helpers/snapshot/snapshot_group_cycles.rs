//! Find job group members that wait on each other at compile, from the same waits publication derives.

use std::collections::{BTreeMap, BTreeSet};

use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_discovery::orchestration_files::models::{FilePosition, OrchestrationInventory};
use sqlbuild_orchestration::declarations::main::group_cycle_message::group_cycle_message;
use sqlbuild_orchestration::declarations::main::group_wait_graph::group_wait_graph;

use crate::graph::models::ProjectSelection;
use crate::project_snapshot::_helpers::snapshot::snapshot_declarations::declared_for;

/// Whether any group declared for `target` lists two or more jobs, so its members could wait on each other.
pub(crate) fn has_groups(inventory: &OrchestrationInventory, target: &str) -> bool {
    inventory
        .groups
        .iter()
        .any(|(_, group)| group.jobs.len() > 1 && declared_for(&group.run.targets, target))
}

/// Each node key and the keys it reads, as publication records them in node `deps`.
fn node_deps(
    graph: &ProjectGraph,
    selected: &ProjectSelection,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for ((kind, name), upstream) in &graph.upstream {
        let reads: &mut BTreeSet<String> = deps.entry(format!("{kind}:{name}")).or_default();
        for (upstream_kind, upstream_name) in upstream {
            reads.insert(format!("{upstream_kind}:{upstream_name}"));
        }
    }
    for (node, upstream) in selected.node_deps() {
        deps.entry(node).or_default().extend(upstream);
    }
    for node in &selected.nodes.nodes {
        if let Some(source) = selected.intermediate_source(&node.kind, &node.name) {
            deps.entry(source)
                .or_default()
                .insert(format!("{}:{}", node.kind, node.name));
        }
    }
    deps
}

/// One `path:line:column: message` per group declared for `target` whose members wait on each other; jobs whose selector fails are left out.
pub(crate) fn group_cycle_failures(
    inventory: &OrchestrationInventory,
    target: &str,
    (graph, selected): (&ProjectGraph, &ProjectSelection),
) -> Vec<String> {
    let tokens = |value: &str| {
        value
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let mut jobs: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (_, job) in &inventory.jobs {
        if !declared_for(&job.run.targets, target) {
            continue;
        }
        let exclude: &str = job.exclude.as_ref().map_or("", |exclude| &exclude.value);
        if let Ok(selection) =
            selected.run_keys(graph, &tokens(&job.select.value), &tokens(exclude))
        {
            jobs.insert(&job.run.name.value, selection);
        }
    }
    let deps: BTreeMap<String, BTreeSet<String>> = node_deps(graph, selected);
    let mut failures: Vec<String> = Vec::new();
    for (path, group) in &inventory.groups {
        if !declared_for(&group.run.targets, target) {
            continue;
        }
        let selections: BTreeMap<String, BTreeSet<String>> = group
            .jobs
            .iter()
            .filter_map(|member| {
                let selection = jobs.get(member.value.as_str())?;
                Some((member.value.clone(), selection.clone()))
            })
            .collect();
        let name: &str = &group.run.name.value;
        if let Some(message) =
            group_cycle_message(name, &group_wait_graph(&selections, &deps).cycles)
        {
            let FilePosition { line, column } = group.run.name.position;
            failures.push(format!("{path}:{line}:{column}: {message}"));
        }
    }
    failures
}
