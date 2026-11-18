//! How job and tag dependency errors name nodes and sets.

use std::collections::HashMap;

use crate::node_set_dependencies::constants::NAMED_NODES;
use crate::node_set_dependencies::models::{GraphNode, NodeSetDependency};

/// `model 'orders'` or `task 'export_orders'`.
pub(crate) fn node_label(node: &GraphNode, kinds: &HashMap<String, String>) -> String {
    match node {
        GraphNode::Sql((kind, name)) => format!("{kind} '{name}'"),
        GraphNode::Python(name) => {
            let kind: &str = kinds.get(name).map_or("node", String::as_str);
            format!("{kind} '{name}'")
        }
    }
}

/// The node's selector name, as an `exclude` lists it.
pub(crate) fn node_name(node: &GraphNode) -> &str {
    match node {
        GraphNode::Sql((_, name)) | GraphNode::Python(name) => name,
    }
}

/// `job 'nightly'` or `tag 'finance'`.
pub(crate) fn set_label(dependency: &NodeSetDependency) -> String {
    format!("{} '{}'", dependency.kind, dependency.name)
}

/// The authoring call that leaves `names` out of the dependency.
pub(crate) fn excluding(dependency: &NodeSetDependency, names: &[&str]) -> String {
    let mut tokens: Vec<&str> = dependency
        .exclude
        .as_deref()
        .map(|exclude| exclude.split_whitespace().collect())
        .unwrap_or_default();
    tokens.extend(names.iter().copied());
    format!(
        "{}(\"{}\", exclude=\"{}\")",
        dependency.kind,
        dependency.name,
        tokens.join(" ")
    )
}

/// Up to [`NAMED_NODES`] labels joined, the rest counted.
pub(crate) fn listed(labels: &[String]) -> String {
    let shown: Vec<&str> = labels
        .iter()
        .take(NAMED_NODES)
        .map(String::as_str)
        .collect();
    match labels.len().saturating_sub(NAMED_NODES) {
        0 => shown.join(", "),
        more => format!("{} and {more} more", shown.join(", ")),
    }
}
