//! A native graph over Python's index mappings, keeping their order.

use crate::graph::_helpers::project::views::graph_from_dicts;
use crate::graph::_helpers::project::views::native_edges;
use crate::graph::_helpers::project::views::native_graph_keys;
use crate::graph::main::nodes::native_key::native_key;
use crate::graph::models::GraphViewMappings;
use crate::graph::models::NativeProjectGraph;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::models::GraphIndexes;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// A native graph over Python's index mappings, keeping their order.
pub fn graph_from_view_mappings(
    mappings: GraphViewMappings<'_, '_>,
) -> PyResult<NativeProjectGraph> {
    let mut names: Vec<(String, ObjectKey)> = Vec::new();
    for item in mapping_items(mappings.all_keys)?.try_iter()? {
        let (name, key): (String, Bound<'_, PyAny>) = item?.extract()?;
        names.push((name, native_key(&key)?));
    }
    let mut tags: Vec<(String, Vec<ObjectKey>)> = Vec::new();
    for item in mapping_items(mappings.tag_index)?.try_iter()? {
        let (tag, keys): (String, Bound<'_, PyAny>) = item?.extract()?;
        tags.push((tag, native_graph_keys(&keys)?));
    }
    let mut paths: Vec<(ObjectKey, String)> = Vec::new();
    for item in mapping_items(mappings.path_index)?.try_iter()? {
        let (key, folder): (Bound<'_, PyAny>, String) = item?.extract()?;
        paths.push((native_key(&key)?, folder));
    }
    graph_from_dicts(GraphIndexes {
        names,
        upstream: native_edges(mappings.upstream)?,
        downstream: native_edges(mappings.downstream)?,
        tags,
        paths,
    })
}
