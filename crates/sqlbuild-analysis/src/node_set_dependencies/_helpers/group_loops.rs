//! Job groups wait job by job: a job dependency that makes two members wait for each other is a cycle.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use sqlbuild_core::graph_walk::main::find_cycle::find_cycle;

use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;
use crate::graph::models::{JobSelection, ProjectGraph};
use crate::node_set_dependencies::_helpers::combined::Combined;
use crate::node_set_dependencies::_helpers::labels::{listed, node_label};
use crate::node_set_dependencies::_helpers::sets::{SetMembers, failure};
use crate::node_set_dependencies::constants::JOB_SET;
use crate::node_set_dependencies::models::GraphNode;

/// Each member job and the nodes it selects.
type Members = Vec<(String, HashSet<GraphNode>)>;
/// Which members each member waits for, each wait with one `(their node, our node)` reason.
type Waits = BTreeMap<String, BTreeMap<String, (GraphNode, GraphNode)>>;

/// The error for the first job group a job dependency makes wait in a loop.
pub(crate) fn group_loop(
    jobs: &ProjectGraph,
    catalog: &BTreeMap<String, JobSelection>,
    (explicit, current): (&Combined, &Combined),
    (sets, kinds): (&[SetMembers<'_>], &HashMap<String, String>),
) -> Option<SelectorError> {
    for (group, selection) in catalog {
        let JobSelection::Group { members } = selection else {
            continue;
        };
        let members: Members = members
            .iter()
            .filter_map(|member| Some((member.clone(), job_nodes(jobs, member, kinds)?)))
            .collect();
        let touched: bool = sets.iter().any(|set| crosses_members(set, &members));
        if !touched || wait_cycle(&waits(&members, explicit)).is_some() {
            continue;
        }
        let after: Waits = waits(&members, current);
        let Some(cycle) = wait_cycle(&after) else {
            continue;
        };
        let before: Waits = waits(&members, explicit);
        return Some(loop_error(group, &cycle, (&before, &after), kinds));
    }
    None
}

/// Whether `set` is a job of the group that a node of another of its jobs depends on.
fn crosses_members(set: &SetMembers<'_>, members: &Members) -> bool {
    let dependant: GraphNode = GraphNode::Python(set.dependant.name.clone());
    let names: Vec<&String> = members.iter().map(|(name, _)| name).collect();
    let elsewhere: bool = members
        .iter()
        .filter(|(name, _)| *name != set.dependency.name)
        .any(|(_, nodes)| nodes.contains(&dependant));
    set.dependency.kind == JOB_SET && names.contains(&&set.dependency.name) && elsewhere
}

/// The nodes `job` selects, Python nodes by name; `None` when it did not resolve.
fn job_nodes(
    jobs: &ProjectGraph,
    job: &str,
    kinds: &HashMap<String, String>,
) -> Option<HashSet<GraphNode>> {
    let Some(Ok(keys)) = jobs.jobs.as_ref()?.get(job) else {
        return None;
    };
    Some(
        keys.iter()
            .map(|key: &ObjectKey| match kinds.get(&key.1) {
                Some(kind) if *kind == key.0 => GraphNode::Python(key.1.clone()),
                _ => GraphNode::Sql(key.clone()),
            })
            .collect(),
    )
}

/// What each member waits for: another member's node feeding its own, maybe through unselected nodes.
fn waits(members: &Members, combined: &Combined) -> Waits {
    let selected: HashSet<&GraphNode> = members.iter().flat_map(|(_, nodes)| nodes).collect();
    let mut found: Waits = BTreeMap::new();
    for (member, own) in members {
        for to in own {
            for up in combined.upstream(to) {
                if own.contains(up) {
                    continue;
                }
                let froms: Vec<GraphNode> = if selected.contains(up) {
                    vec![up.clone()]
                } else {
                    combined
                        .reach_upstream([up], |node| !selected.contains(node))
                        .into_iter()
                        .filter(|node| selected.contains(node))
                        .collect()
                };
                for from in froms {
                    if own.contains(&from) {
                        continue;
                    }
                    for (other, theirs) in members {
                        if other != member && theirs.contains(&from) {
                            found
                                .entry(member.clone())
                                .or_default()
                                .entry(other.clone())
                                .or_insert_with(|| (from.clone(), to.clone()));
                        }
                    }
                }
            }
        }
    }
    found
}

/// The members of a wait cycle, the first waiting for the second and the last for the first.
fn wait_cycle(waits: &Waits) -> Option<Vec<String>> {
    let path: Vec<String> = find_cycle(waits.keys().cloned(), |member: &String| {
        waits
            .get(member)
            .map(|waited| waited.keys().cloned().collect::<Vec<String>>())
            .unwrap_or_default()
    })?;
    let last: &String = path.last()?;
    let start: usize = path.iter().position(|member| member == last)?;
    Some(path[start..path.len() - 1].to_vec())
}

fn loop_error(
    group: &str,
    cycle: &[String],
    (before, after): (&Waits, &Waits),
    kinds: &HashMap<String, String>,
) -> SelectorError {
    let mut steps: Vec<String> = Vec::new();
    let mut closers: BTreeSet<String> = BTreeSet::new();
    let mut dependants: BTreeSet<String> = BTreeSet::new();
    for (index, member) in cycle.iter().enumerate() {
        let waited: &String = &cycle[(index + 1) % cycle.len()];
        let Some((from, to)) = after.get(member).and_then(|waits| waits.get(waited)) else {
            continue;
        };
        let added: bool = before
            .get(member)
            .is_none_or(|waits| !waits.contains_key(waited));
        let (from, to): (String, String) = (node_label(from, kinds), node_label(to, kinds));
        if added {
            steps.push(format!(
                "'{member}' waits for '{waited}' ({to} depends on job '{waited}', which ends in {from})"
            ));
            dependants.insert(format!("{to} out of '{member}'"));
        } else {
            steps.push(format!(
                "'{member}' waits for '{waited}' ('{member}''s {to} reads {from})"
            ));
            closers.insert(to);
        }
    }
    let closers: Vec<String> = closers.into_iter().collect();
    let dependants: Vec<String> = dependants.into_iter().collect();
    failure(
        format!(
            "a job dependency makes job group '{group}' wait in a loop: {}",
            steps.join("; ")
        ),
        Some(format!(
            "a job group runs each job once its jobs upstream have finished, so these jobs would \
             wait for each other; move {} to another job, or move {}",
            listed(&closers),
            listed(&dependants)
        )),
    )
}
