//! Build group selections and node lineage for the wait graph tests.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use uuid::Uuid;

use crate::declarations::main::group_dependencies::group_dependencies;
use crate::declarations::models::JobDefinition;
use crate::declarations::tests::test_types::Pairs;
use crate::snapshots::models::{SnapshotInput, SnapshotNode};
use crate::store::errors::ControlError;

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

/// A snapshot whose models read the nodes `lineage` lists.
pub(crate) fn input(lineage: &[(&str, Vec<&str>)]) -> SnapshotInput {
    SnapshotInput {
        location_id: Uuid::nil(),
        target: "development".into(),
        generation: 1,
        git_revision: None,
        git_dirty: false,
        git_author: None,
        image_id: None,
        environment_hash: "orders-environment".into(),
        sqb_version: "1.0.0".into(),
        nodes: lineage
            .iter()
            .map(|(key, deps)| {
                (
                    (*key).to_owned(),
                    SnapshotNode {
                        kind: "model".into(),
                        definition: json!({"deps": deps}),
                        raw_sql: None,
                        compiled_sql: None,
                        python_source: None,
                    },
                )
            })
            .collect(),
        documents: BTreeMap::new(),
    }
}

/// Member jobs selecting the nodes `selections` lists.
pub(crate) fn jobs(selections: &[(&str, Vec<&str>)]) -> BTreeMap<String, JobDefinition> {
    selections
        .iter()
        .map(|(name, selection)| {
            let definition =
                json!({"description": format!("Build {name}"), "selection": selection});
            (
                (*name).to_owned(),
                serde_json::from_value(definition).expect("job definition"),
            )
        })
        .collect()
}

/// The group waits of members `selections` over `lineage`.
pub(crate) fn waits(
    lineage: &[(&str, Vec<&str>)],
    selections: &[(&str, Vec<&str>)],
) -> Result<BTreeMap<String, BTreeSet<String>>, ControlError> {
    let jobs = jobs(selections);
    let members: BTreeSet<String> = jobs.keys().cloned().collect();
    group_dependencies(("orders_job_group", &input(lineage)), &jobs, &members)
}
