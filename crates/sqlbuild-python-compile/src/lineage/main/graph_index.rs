//! The lineage index of a compiled `ProjectGraph`; every named key has a node.

use crate::lineage::_helpers::lineage::lineage_index::ordered_edges;
use crate::lineage::_helpers::lineage::lineage_index::project_nodes;
use crate::lineage::main::lineage_key::lineage_key;
use crate::lineage::main::lineage_keys::lineage_keys;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::lineage_command::models::{LineageIndex, LineageKey, LineageNode};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::HashMap;

/// The lineage index of a compiled `ProjectGraph`; every named key has a node.
pub fn graph_index(graph: &Bound<'_, PyAny>) -> PyResult<LineageIndex> {
    let views: Bound<'_, PyAny> = native_field(graph, "lineage_views")?;
    let mut all_keys: Vec<(String, LineageKey)> = Vec::new();
    for item in native_field(&views, "all_keys")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, key): (String, Bound<'_, PyAny>) = item?.extract()?;
        all_keys.push((name, lineage_key(&key)?));
    }
    let mut tags: Vec<(String, Vec<LineageKey>)> = Vec::new();
    for item in native_field(&views, "tag_index")?
        .call_method0("items")?
        .try_iter()?
    {
        let (tag, keys): (String, Bound<'_, PyAny>) = item?.extract()?;
        tags.push((tag, lineage_keys(&keys)?));
    }
    let mut paths: Vec<(LineageKey, String)> = Vec::new();
    for item in native_field(&views, "path_index")?
        .call_method0("items")?
        .try_iter()?
    {
        let (key, folder): (Bound<'_, PyAny>, String) = item?.extract()?;
        paths.push((lineage_key(&key)?, folder));
    }
    let mut nodes: HashMap<LineageKey, LineageNode> =
        project_nodes(&native_field(graph, "project")?)?;
    for (_, key) in &all_keys {
        nodes.entry(key.clone()).or_insert_with(|| LineageNode {
            key: key.clone(),
            relative_path: None,
            qualified_name: None,
        });
    }
    Ok(LineageIndex {
        nodes,
        upstream: ordered_edges(&native_field(&views, "upstream_deps")?)?,
        downstream: ordered_edges(&native_field(&views, "downstream_deps")?)?,
        tags,
        paths,
        all_keys,
    })
}
