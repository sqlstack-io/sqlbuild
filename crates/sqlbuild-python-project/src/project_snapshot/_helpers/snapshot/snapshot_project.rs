//! Project metadata stays native until its immutable snapshot is sent over HTTP.

use std::collections::BTreeMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyResult;
use serde_json::Value;
use sqlbuild_orchestration::snapshots::models::SnapshotNode;

/// Duplicate resource identities must fail, rather than silently replacing an earlier node.
pub(crate) fn insert_node(
    mut nodes: BTreeMap<String, SnapshotNode>,
    key: String,
    node: SnapshotNode,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    if nodes.contains_key(&key) {
        return Err(PyValueError::new_err(format!(
            "duplicate snapshot node identity {key}"
        )));
    }
    nodes.insert(key, node);
    Ok(nodes)
}

pub(crate) fn dependency_keys(deps: &[(String, String)]) -> Vec<String> {
    deps.iter()
        .map(|(kind, name)| format!("{kind}:{name}"))
        .collect()
}

pub(crate) fn destination(value: &sqlbuild_project::models::DestinationFact) -> Value {
    serde_json::json!({
        "database": value.database,
        "schema": value.schema,
        "name": value.name,
        "qualified_name": value.qualified_name,
        "logical_database": value.logical_database,
        "logical_schema": value.logical_schema,
    })
}
