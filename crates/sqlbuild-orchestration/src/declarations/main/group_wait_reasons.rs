//! Why each group member waits for another, node by node, from snapshot lineage.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::_helpers::group_waits::{selected_upstream, upstream_keys};
use crate::declarations::models::{JobDefinition, WaitReason};
use crate::snapshots::models::SnapshotInput;

/// Each member's links from a node of a member it waits for to one of its own; lineage through another member's node is that member's.
pub fn group_wait_reasons(
    input: &SnapshotInput,
    jobs: &BTreeMap<String, JobDefinition>,
    waits: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, Vec<WaitReason>> {
    let selected: BTreeSet<&str> = waits
        .keys()
        .flat_map(|member| jobs[member].selection.iter().map(String::as_str))
        .collect();
    let mut reached: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut reasons: BTreeMap<String, Vec<WaitReason>> = BTreeMap::new();
    for (member, upstream_jobs) in waits {
        let own = &jobs[member].selection;
        let mut found: Vec<WaitReason> = Vec::new();
        for upstream_job in upstream_jobs {
            let theirs = &jobs[upstream_job].selection;
            let mut seen: BTreeSet<(String, &str)> = BTreeSet::new();
            for to in own {
                for up in upstream_keys(input, to) {
                    if own.contains(up) {
                        continue;
                    }
                    let froms: BTreeSet<String> = if selected.contains(up) {
                        BTreeSet::from([up.to_owned()])
                    } else {
                        reached
                            .entry(up.to_owned())
                            .or_insert_with(|| selected_upstream(input, &selected, up))
                            .clone()
                    };
                    for from in froms {
                        if theirs.contains(&from)
                            && !own.contains(&from)
                            && seen.insert((from.clone(), to.as_str()))
                        {
                            found.push(WaitReason {
                                upstream_job: upstream_job.clone(),
                                from_node: from,
                                to_node: to.clone(),
                            });
                        }
                    }
                }
            }
        }
        if !found.is_empty() {
            reasons.insert(member.clone(), found);
        }
    }
    reasons
}
