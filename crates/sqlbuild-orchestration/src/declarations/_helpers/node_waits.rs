//! Find a wait cycle among a group's nodes when members wait node by node.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::_helpers::group_waits::upstream_keys;
use crate::declarations::constants::PHASE_NODE_KINDS;
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

/// Whether a node runs in a Python or load phase of its member's build, where waiting would hold the member's other nodes back.
#[must_use]
pub(crate) fn phase_node(key: &str) -> bool {
    key.split_once(':')
        .is_some_and(|(kind, _)| PHASE_NODE_KINDS.contains(&kind))
}

/// Each node's nearest upstream nodes some member selects, reached through nodes none selects.
fn selected_inputs<'a>(
    input: &'a SnapshotInput,
    owners: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    memo: &mut BTreeMap<&'a str, BTreeSet<&'a str>>,
    key: &'a str,
) -> BTreeSet<&'a str> {
    if let Some(found) = memo.get(key) {
        return found.clone();
    }
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
    memo.insert(key, found.clone());
    found
}

/// The cross-member inputs of each node of each member: `(owning member, node)` pairs it reads.
fn cross_member_inputs<'a>(group: &GroupMembers<'a>) -> CrossInputs<'a> {
    let owners = owners(group);
    let mut memo: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut inputs: CrossInputs<'a> = BTreeMap::new();
    for member in group.edges.keys() {
        let own = &group.jobs[member].selection;
        let member_inputs = inputs.entry(member.as_str()).or_default();
        for node in own {
            for upstream in selected_inputs(group.input, &owners, &mut memo, node) {
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

/// The waits that close a cycle, each naming its nodes, or `None` when the group can run.
pub(crate) fn node_wait_cycle(group: &GroupMembers<'_>) -> Option<String> {
    let owners = owners(group);
    let mut memo: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let cross = cross_member_inputs(group);
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
        for node in own {
            let vertex = Vertex::Node(member.as_str(), node.as_str());
            let mut node_waits: BTreeSet<Vertex<'_>> = BTreeSet::new();
            for upstream in selected_inputs(group.input, &owners, &mut memo, node) {
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
            } else {
                node_waits.extend(upstream_jobs.iter().map(|job| Vertex::JobEnd(job)));
            }
            waits.entry(vertex).or_default().extend(node_waits);
        }
    }
    cycle(waits)
}

/// Peel vertices with nothing left to wait for; any left over wait on each other, so following waits closes a loop.
fn cycle(mut remaining: BTreeMap<Vertex<'_>, BTreeSet<Vertex<'_>>>) -> Option<String> {
    loop {
        let ready: BTreeSet<Vertex<'_>> = remaining
            .iter()
            .filter(|(_, waits)| waits.iter().all(|wait| !remaining.contains_key(wait)))
            .map(|(vertex, _)| *vertex)
            .collect();
        if ready.is_empty() {
            break;
        }
        remaining.retain(|vertex, _| !ready.contains(vertex));
    }
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
    if path.is_empty() {
        return None;
    }
    Some(
        path.iter()
            .zip(path.iter().cycle().skip(1))
            .map(|(vertex, wait)| format!("{} waits for {}", vertex.describe(), wait.describe()))
            .collect::<Vec<_>>()
            .join("; "),
    )
}
