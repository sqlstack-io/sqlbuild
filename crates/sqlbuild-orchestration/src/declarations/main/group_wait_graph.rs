//! Derive group member waits from node lineage and report every set of members that wait on each other.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::_helpers::group_waits::{
    cycle_path, member_waits, named_cycle, strongly_connected,
};
use crate::declarations::models::{GroupCycle, GroupWaits};

/// Each member waits for every other member that builds a node its nodes read, unless it builds that node too.
pub fn group_wait_graph(
    selections: &BTreeMap<String, BTreeSet<String>>,
    deps: &BTreeMap<String, BTreeSet<String>>,
) -> GroupWaits {
    let mut owners: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (name, selection) in selections {
        for key in selection {
            owners.entry(key).or_default().insert(name);
        }
    }
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut reasons: BTreeMap<(String, String), (String, String)> = BTreeMap::new();
    for (name, selection) in selections {
        let waits: BTreeMap<String, (String, String)> =
            member_waits((name, selection), deps, &owners);
        edges.insert(name.clone(), waits.keys().cloned().collect());
        for (dependency, reason) in waits {
            reasons.insert((name.clone(), dependency), reason);
        }
    }
    let mut cycles: Vec<GroupCycle> = Vec::new();
    for members in strongly_connected(&edges) {
        cycles.push(named_cycle(
            &cycle_path(&members, &edges),
            &reasons,
            &owners,
        ));
    }
    GroupWaits {
        edges,
        reasons,
        cycles,
    }
}
