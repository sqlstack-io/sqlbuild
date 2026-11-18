use std::collections::HashMap;

use crate::python_plan::models::LifecycleNode;
use crate::python_plan::tests::test_types::LifecycleNodeSpec;

pub(crate) fn edges(rows: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
    rows.iter()
        .map(|(name, upstream)| {
            (
                (*name).to_owned(),
                upstream.iter().map(|value| (*value).to_owned()).collect(),
            )
        })
        .collect()
}

pub(crate) fn lifecycle_nodes(rows: &[LifecycleNodeSpec]) -> HashMap<String, LifecycleNode> {
    rows.iter()
        .map(|(name, kind, sql_deps)| {
            (
                (*name).to_owned(),
                LifecycleNode {
                    kind: (*kind).to_owned(),
                    sql_deps: sql_deps
                        .iter()
                        .map(|(kind, name)| ((*kind).to_owned(), (*name).to_owned()))
                        .collect(),
                },
            )
        })
        .collect()
}

/// Owned copies of `values`.
pub(crate) fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|name| (*name).to_owned()).collect()
}
