//! The native project graph's indexes as the Python dicts graph consumers read, and back.

use crate::graph::main::nodes::native_key::native_key;
use crate::graph::main::project::edge_dict::edge_dict;
use crate::graph::main::project::graph_from_view_mappings::graph_from_view_mappings;
use crate::graph::models::GraphViewMappings;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::graph_from_indexes::graph_from_indexes;
use sqlbuild_analysis::graph::models::{GraphIndexes, ProjectGraph};

use crate::graph::models::KeyCache;
use crate::graph::models::NativeProjectGraph;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::RecordId;

/// The native graph's pairs for `keys`, in order.
#[pyfunction]
pub(crate) fn native_graph_keys(keys: &Bound<'_, PyAny>) -> PyResult<Vec<ObjectKey>> {
    keys.try_iter()?.map(|key| native_key(&key?)).collect()
}

/// Selector name to key, in project order, without the other graph views.
#[pyfunction]
pub(crate) fn lineage_graph_names<'py>(
    py: Python<'py>,
    graph: PyRef<'py, NativeProjectGraph>,
) -> PyResult<Bound<'py, PyDict>> {
    let cache: KeyCache<'py> = KeyCache::new(py)?;
    let names: Bound<'py, PyDict> = PyDict::new(py);
    for (name, key) in &graph.graph().names {
        names.set_item(name, cache.key(key)?)?;
    }
    Ok(names)
}

/// The native graph's indexes as `LineageGraphViews` dicts, in project order.
pub(crate) fn graph_views<'py>(
    py: Python<'py>,
    graph: &ProjectGraph,
) -> PyResult<Bound<'py, PyAny>> {
    let cache: KeyCache<'py> = KeyCache::new(py)?;
    let tags: Bound<'py, PyDict> = PyDict::new(py);
    for (tag, keys) in &graph.tags {
        tags.set_item(tag, PyFrozenSet::new(py, cache.keys(keys)?)?)?;
    }
    let paths: Bound<'py, PyDict> = PyDict::new(py);
    for (key, folder) in &graph.paths {
        paths.set_item(cache.key(key)?, folder)?;
    }
    let names: Bound<'py, PyDict> = PyDict::new(py);
    for (name, key) in &graph.names {
        names.set_item(name, cache.key(key)?)?;
    }
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("upstream_deps", edge_dict(&cache, &graph.upstream)?)?;
    keywords.set_item("downstream_deps", edge_dict(&cache, &graph.downstream)?)?;
    keywords.set_item("tag_index", tags)?;
    keywords.set_item("path_index", paths)?;
    keywords.set_item("all_keys", names)?;
    new_record(py, RecordId::LineageGraphViews, (), Some(&keywords))
}

/// The graph's lineage edges and selector indexes as dicts, in project order.
#[pyfunction]
pub(crate) fn lineage_graph_views<'py>(
    py: Python<'py>,
    graph: PyRef<'py, NativeProjectGraph>,
) -> PyResult<Bound<'py, PyAny>> {
    graph_views(py, graph.graph())
}

pub(crate) fn native_edges(
    mapping: &Bound<'_, PyAny>,
) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
    let mut edges: Vec<(ObjectKey, Vec<ObjectKey>)> = Vec::new();
    for item in mapping_items(mapping)?.try_iter()? {
        let (key, deps): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
        edges.push((native_key(&key)?, native_graph_keys(&deps)?));
    }
    Ok(edges)
}

pub(crate) fn graph_from_dicts(indexes: GraphIndexes) -> PyResult<NativeProjectGraph> {
    compiler_guard(|| Ok(NativeProjectGraph::new(graph_from_indexes(indexes))))
}

/// A native graph over these indexes, keeping their order.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn native_graph_from_views(
    request: Option<&Bound<'_, PyDict>>,
) -> PyResult<NativeProjectGraph> {
    let Some(request) = request else {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "native_graph_from_views() missing keyword arguments",
        ));
    };
    let field = |name: &str| -> PyResult<Bound<'_, PyAny>> {
        request.get_item(name)?.ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err(format!(
                "native_graph_from_views() missing keyword argument '{name}'"
            ))
        })
    };
    graph_from_view_mappings(GraphViewMappings {
        all_keys: &field("all_keys")?,
        upstream: &field("upstream")?,
        downstream: &field("downstream")?,
        tag_index: &field("tag_index")?,
        path_index: &field("path_index")?,
    })
}
