//! The nodes each published audit and SQL test checks, read from the snapshot's nodes.

use std::collections::BTreeMap;

use serde_json::Value;
use sqlbuild_orchestration::snapshots::models::SnapshotNode;

use crate::job_settings::constants::SQL_TEST_NODE_KIND;

/// Each audit and SQL test among `nodes`, with the node keys it checks.
#[must_use]
pub fn attached_nodes(nodes: &BTreeMap<String, SnapshotNode>) -> BTreeMap<String, Vec<String>> {
    let mut attached: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (key, node) in nodes {
        let definition: &Value = &node.definition;
        let checked: Vec<String> = match node.kind.as_str() {
            "audit" => definition
                .get("target")
                .and_then(|target| {
                    Some(format!(
                        "{}:{}",
                        target.get("kind")?.as_str()?,
                        target.get("name")?.as_str()?
                    ))
                })
                .into_iter()
                .collect(),
            SQL_TEST_NODE_KIND => definition
                .pointer("/test/payload/expected_model_names")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|name| format!("model:{name}"))
                .collect(),
            _ => continue,
        };
        attached.insert(key.clone(), checked);
    }
    attached
}
