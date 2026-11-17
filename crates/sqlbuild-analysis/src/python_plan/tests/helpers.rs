use std::collections::HashMap;

use crate::python_plan::models::LifecycleNode;

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

pub(crate) fn lifecycle_nodes(rows: &[(&str, &str)]) -> HashMap<String, LifecycleNode> {
    rows.iter()
        .map(|(name, kind)| {
            (
                (*name).to_owned(),
                LifecycleNode {
                    kind: (*kind).to_owned(),
                },
            )
        })
        .collect()
}
