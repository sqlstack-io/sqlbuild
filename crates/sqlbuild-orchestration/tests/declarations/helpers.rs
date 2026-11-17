//! Build group selections and node lineage for the wait graph tests.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::tests::test_types::Pairs;

pub(crate) fn map(pairs: Pairs) -> BTreeMap<String, BTreeSet<String>> {
    pairs
        .iter()
        .map(|(key, values)| {
            (
                (*key).to_owned(),
                values.iter().map(|value| (*value).to_owned()).collect(),
            )
        })
        .collect()
}

/// `members` jobs over one acyclic chain of nodes, each node reading the one before it, where the first job also builds a tail reading the last node.
pub(crate) fn chained_group(
    members: usize,
    nodes_per_member: usize,
) -> (
    BTreeMap<String, BTreeSet<String>>,
    BTreeMap<String, BTreeSet<String>>,
) {
    let node = |index: usize| format!("model:m{index}");
    let last: usize = members * nodes_per_member - 1;
    let mut selections: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for member in 0..members {
        let first: usize = member * nodes_per_member;
        selections.insert(
            format!("job_{member:03}"),
            (first..first + nodes_per_member).map(node).collect(),
        );
    }
    selections
        .entry("job_000".to_owned())
        .or_default()
        .insert("model:tail".to_owned());
    let mut deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for index in 1..=last {
        deps.insert(node(index), BTreeSet::from([node(index - 1)]));
    }
    deps.insert("model:tail".to_owned(), BTreeSet::from([node(last)]));
    (selections, deps)
}
