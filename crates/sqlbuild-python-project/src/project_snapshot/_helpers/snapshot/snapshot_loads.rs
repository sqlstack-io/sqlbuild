//! Each load as a snapshot node: its settings, the sources it fills and its fingerprint.

use std::collections::BTreeMap;

use pyo3::prelude::{PyAnyMethods, PyResult, Python};
use serde_json::json;
use sqlbuild_orchestration::snapshots::models::SnapshotNode;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::compiled_project::main::plain_value::plain_value;
use crate::compiled_views::models::NativeProjectView;
use crate::project_snapshot::_helpers::snapshot::snapshot_project::insert_node;

/// `nodes` with a `load:<name>` node for each load the project compiles.
pub(crate) fn load_nodes(
    py: Python<'_>,
    project: &NativeProjectView,
    mut nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    for load in project.loads(py)?.try_iter()? {
        let load = load?;
        let name: String = native_field(&load, "name")?.extract()?;
        let sources: Vec<String> = native_field(&load, "sources")?.extract()?;
        nodes = insert_node(
            nodes,
            format!("load:{name}"),
            SnapshotNode {
                kind: "load".into(),
                definition: json!({
                    "name": name,
                    "path": native_field(&load, "relative_path")?.extract::<String>()?,
                    "settings": plain_value(&native_field(&load, "settings")?)?,
                    "sources": sources,
                    "fingerprint": native_field(&load, "fingerprint")?.extract::<String>()?,
                    "deps": Vec::<String>::new(),
                }),
                raw_sql: None,
                compiled_sql: None,
                python_source: None,
            },
        )?;
    }
    Ok(nodes)
}
