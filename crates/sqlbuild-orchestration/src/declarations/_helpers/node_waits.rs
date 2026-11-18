//! Find a wait cycle among a group's nodes when members wait node by node.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::_helpers::group_waits::upstream_keys;
use crate::declarations::main::phase_node::phase_node;
use crate::declarations::models::JobDefinition;
use crate::snapshots::models::SnapshotInput;

/// What a node of a group run waits for: another member's node, the end of a member job, or the start of a member.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Vertex<'a> {
    Node(&'a str, &'a str),
    JobEnd(&'a str),
    MemberStart(&'a str),
}

impl Vertex<'_> {
    fn describe(self) -> String {
        match self {
            Self::Node(member, node) => format!("{node} ({member})"),
            Self::JobEnd(member) => format!("the end of job {member}"),
            Self::MemberStart(member) => format!("the start of job {member}"),
        }
    }
}

/// Each member's nodes and, per node, the `(owning member, node)` pairs it reads from other members.
type CrossInputs<'a> = BTreeMap<&'a str, BTreeMap<&'a str, BTreeSet<(&'a str, &'a str)>>>;

/// A group's members, what each selects and which nodes each member's selection is owned by.
pub(crate) struct GroupMembers<'a> {
    pub(crate) input: &'a SnapshotInput,
    pub(crate) jobs: &'a BTreeMap<String, JobDefinition>,
    pub(crate) edges: &'a BTreeMap<String, BTreeSet<String>>,
    pub(crate) node_waits: &'a BTreeSet<String>,
}

/// Each selected node's nearest upstream nodes some member selects, reached through nodes none selects.
fn selected_inputs<'a>(
    input: &'a SnapshotInput,
    owners: &BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> BTreeMap<&'a str, BTreeSet<&'a str>> {
    let mut inputs: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for key in owners.keys() {
        let mut found: BTreeSet<&str> = BTreeSet::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut pending: Vec<&str> = upstream_keys(input, key);
        while let Some(upstream) = pending.pop() {
            if !seen.insert(upstream) {
                continue;
            }
            if owners.contains_key(upstream) {
                found.insert(upstream);
            } else {
                pending.extend(upstream_keys(input, upstream));
            }
        }
        inputs.insert(key, found);
    }
    inputs
}

/// The cross-member inputs of each node of each member: `(owning member, node)` pairs it reads.
fn cross_member_inputs<'a>(group: &GroupMembers<'a>) -> CrossInputs<'a> {
    let owners = owners(group);
    let selected = selected_inputs(group.input, &owners);
    let mut inputs: CrossInputs<'a> = BTreeMap::new();
    for member in group.edges.keys() {
        let own = &group.jobs[member].selection;
        let member_inputs = inputs.entry(member.as_str()).or_default();
        for node in own {
            for upstream in selected.get(node.as_str()).into_iter().flatten().copied() {
                if own.contains(upstream) {
                    continue;
                }
                for owner in &owners[upstream] {
                    member_inputs
                        .entry(node.as_str())
                        .or_default()
                        .insert((owner, upstream));
                }
            }
        }
    }
    inputs
}

fn owners<'a>(group: &GroupMembers<'a>) -> BTreeMap<&'a str, BTreeSet<&'a str>> {
    let mut owners: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for member in group.edges.keys() {
        for key in &group.jobs[member].selection {
            owners
                .entry(key.as_str())
                .or_default()
                .insert(member.as_str());
        }
    }
    owners
}

/// Every vertex and what it waits for: each node its own upstream nodes and, for a member that waits per node, its inputs from other members; a member that waits for jobs, or for `serialized` members, waits for their ends.
fn wait_graph<'a>(
    group: &GroupMembers<'a>,
    serialized: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<Vertex<'a>, BTreeSet<Vertex<'a>>> {
    let owners = owners(group);
    let selected = selected_inputs(group.input, &owners);
    let cross = cross_member_inputs(group);
    let no_jobs: BTreeSet<String> = BTreeSet::new();
    let mut waits: BTreeMap<Vertex<'_>, BTreeSet<Vertex<'_>>> = BTreeMap::new();
    for (member, upstream_jobs) in group.edges {
        let own = &group.jobs[member].selection;
        let per_node = group.node_waits.contains(member);
        let member_inputs = cross.get(member.as_str());
        let starts_late = per_node
            && member_inputs.is_some_and(|inputs| inputs.keys().any(|node| phase_node(node)));
        waits.entry(Vertex::JobEnd(member)).or_default().extend(
            own.iter()
                .map(|node| Vertex::Node(member.as_str(), node.as_str())),
        );
        if starts_late {
            let start = waits.entry(Vertex::MemberStart(member)).or_default();
            for (node, inputs) in member_inputs.into_iter().flatten() {
                if phase_node(node) {
                    start.extend(inputs.iter().map(|(owner, key)| Vertex::Node(owner, key)));
                }
            }
        }
        let mut job_ends: Vec<&str> = Vec::new();
        let ended: &BTreeSet<String> = if per_node {
            serialized.get(member).unwrap_or(&no_jobs)
        } else {
            upstream_jobs
        };
        for job in ended {
            if let Some((job, _)) = group.edges.get_key_value(job) {
                job_ends.push(job.as_str());
            }
        }
        for node in own {
            let vertex = Vertex::Node(member.as_str(), node.as_str());
            let mut node_waits: BTreeSet<Vertex<'_>> = BTreeSet::new();
            for upstream in selected.get(node.as_str()).into_iter().flatten().copied() {
                if own.contains(upstream) {
                    node_waits.insert(Vertex::Node(member.as_str(), upstream));
                }
            }
            if per_node {
                if let Some(inputs) = member_inputs.and_then(|inputs| inputs.get(node.as_str())) {
                    node_waits.extend(inputs.iter().map(|(owner, key)| Vertex::Node(owner, key)));
                }
                if starts_late {
                    node_waits.insert(Vertex::MemberStart(member));
                }
            }
            node_waits.extend(job_ends.iter().map(|job| Vertex::JobEnd(job)));
            waits.entry(vertex).or_default().extend(node_waits);
        }
    }
    waits
}

/// For each member that waits per node, the members it waits for as whole jobs because they share a pool and it reads their nodes; or the waits that close a cycle, naming the nodes.
pub(crate) fn node_wait_plan(
    group: &GroupMembers<'_>,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut serialized: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    loop {
        let waits = wait_graph(group, &serialized);
        let order = match peel(&waits) {
            Ok(order) => order,
            Err(remaining) => {
                let cycle = describe(&remaining);
                return Err(if serialized.is_empty() {
                    cycle
                } else {
                    format!(
                        "{cycle} (members that share a pool run one at a time, so one that reads \
                         another's nodes waits for that whole job)"
                    )
                });
            }
        };
        let mut reached: BTreeMap<Vertex<'_>, BTreeSet<&str>> = BTreeMap::new();
        for vertex in &order {
            let mut members: BTreeSet<&str> = BTreeSet::new();
            for wait in &waits[vertex] {
                match wait {
                    Vertex::Node(owner, _) | Vertex::JobEnd(owner) => {
                        members.insert(owner);
                    }
                    Vertex::MemberStart(_) => {}
                }
                members.extend(reached.get(wait).into_iter().flatten().copied());
            }
            reached.insert(*vertex, members);
        }
        let mut added = false;
        for member in group.node_waits {
            let pools = &group.jobs[member].pools;
            let mut needs: BTreeSet<&str> = BTreeSet::new();
            for node in &group.jobs[member].selection {
                needs.extend(
                    reached
                        .get(&Vertex::Node(member.as_str(), node.as_str()))
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }
            for other in needs {
                if other != member
                    && group.jobs[other]
                        .pools
                        .keys()
                        .any(|pool| pools.contains_key(pool))
                    && serialized
                        .entry(member.clone())
                        .or_default()
                        .insert(other.to_owned())
                {
                    added = true;
                }
            }
        }
        serialized.retain(|_, jobs| !jobs.is_empty());
        if !added {
            return Ok(serialized);
        }
    }
}

/// Vertices in an order where each comes after everything it waits for, or the ones left waiting on each other.
fn peel<'a>(
    waits: &BTreeMap<Vertex<'a>, BTreeSet<Vertex<'a>>>,
) -> Result<Vec<Vertex<'a>>, BTreeMap<Vertex<'a>, BTreeSet<Vertex<'a>>>> {
    let mut remaining = waits.clone();
    let mut order: Vec<Vertex<'_>> = Vec::new();
    loop {
        let left: BTreeSet<Vertex<'_>> = remaining.keys().copied().collect();
        let mut ready: Vec<Vertex<'_>> = Vec::new();
        for (vertex, waits) in &remaining {
            if waits.is_disjoint(&left) {
                ready.push(*vertex);
            }
        }
        if ready.is_empty() {
            break;
        }
        for vertex in &ready {
            remaining.remove(vertex);
        }
        order.extend(ready);
    }
    if remaining.is_empty() {
        Ok(order)
    } else {
        Err(remaining)
    }
}

/// Following waits among vertices that all wait on each other closes a loop; name each wait in it.
fn describe(remaining: &BTreeMap<Vertex<'_>, BTreeSet<Vertex<'_>>>) -> String {
    let mut path: Vec<Vertex<'_>> = Vec::new();
    let mut current: Option<Vertex<'_>> = remaining.keys().next().copied();
    while let Some(vertex) = current {
        if let Some(start) = path.iter().position(|seen| *seen == vertex) {
            path.drain(..start);
            break;
        }
        path.push(vertex);
        current = remaining[&vertex]
            .iter()
            .find(|wait| remaining.contains_key(wait))
            .copied();
    }
    path.iter()
        .zip(path.iter().cycle().skip(1))
        .map(|(vertex, wait)| format!("{} waits for {}", vertex.describe(), wait.describe()))
        .collect::<Vec<_>>()
        .join("; ")
}
