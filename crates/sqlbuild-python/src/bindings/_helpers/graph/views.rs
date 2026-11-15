//! The native project graph's indexes as the Python dicts graph consumers read, and back.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRef, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::graph_from_indexes::graph_from_indexes;
use sqlbuild_analysis::graph::models::{GraphIndexes, ProjectGraph};

use crate::bindings::_helpers::graph::keys::{KeyCache, native_key};
use crate::bindings::_helpers::graph::project_graph::NativeProjectGraph;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::RecordId;

/// The native graph's pairs for `keys`, in order.
#[pyfunction]
fn native_graph_keys(keys: &Bound<'_, PyAny>) -> PyResult<Vec<ObjectKey>> {
    keys.try_iter()?.map(|key| native_key(&key?)).collect()
}

/// `CompiledObjectKey`s for native graph key pairs.
#[pyfunction]
pub(crate) fn compiled_graph_keys<'py>(
    py: Python<'py>,
    keys: Vec<ObjectKey>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let cache: KeyCache<'py> = KeyCache::new(py)?;
    PyFrozenSet::new(py, cache.keys(&keys)?)
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

pub(crate) fn edge_dict<'py>(
    cache: &KeyCache<'py>,
    edges: &[(ObjectKey, Vec<ObjectKey>)],
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = cache.py();
    let dict: Bound<'py, PyDict> = PyDict::new(py);
    for (key, deps) in edges {
        dict.set_item(cache.key(key)?, PyTuple::new(py, cache.keys(deps)?)?)?;
    }
    Ok(dict)
}

/// The native graph's indexes as `LineageGraphViews` dicts, in project order.
fn graph_views<'py>(py: Python<'py>, graph: &ProjectGraph) -> PyResult<Bound<'py, PyAny>> {
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

/// The project's lineage edges and selector indexes, from its native graph.
#[pyfunction]
pub(crate) fn project_lineage_views<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let graph: PyRef<'py, NativeProjectGraph> =
        native_field(project, "lineage_graph")?.extract()?;
    graph_views(py, graph.graph())
}

fn native_edges(mapping: &Bound<'_, PyAny>) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
    let mut edges: Vec<(ObjectKey, Vec<ObjectKey>)> = Vec::new();
    for item in mapping_items(mapping)?.try_iter()? {
        let (key, deps): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
        edges.push((native_key(&key)?, native_graph_keys(&deps)?));
    }
    Ok(edges)
}

fn graph_from_dicts(indexes: GraphIndexes) -> PyResult<NativeProjectGraph> {
    compiler_guard(|| Ok(NativeProjectGraph::new(graph_from_indexes(indexes))))
}

/// A native graph over these indexes, keeping their order.
#[pyfunction]
#[pyo3(signature = (**request))]
fn native_graph_from_views(request: Option<&Bound<'_, PyDict>>) -> PyResult<NativeProjectGraph> {
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

/// Python's selector name, edge, tag and path mappings.
pub(crate) struct GraphViewMappings<'a, 'py> {
    pub(crate) all_keys: &'a Bound<'py, PyAny>,
    pub(crate) upstream: &'a Bound<'py, PyAny>,
    pub(crate) downstream: &'a Bound<'py, PyAny>,
    pub(crate) tag_index: &'a Bound<'py, PyAny>,
    pub(crate) path_index: &'a Bound<'py, PyAny>,
}

/// A native graph over Python's index mappings, keeping their order.
pub(crate) fn graph_from_view_mappings(
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

/// A native graph holding these edges in their order, with no selector indexes.
#[pyfunction]
#[pyo3(signature = (*, upstream, downstream))]
pub(crate) fn native_graph_from_edges(
    upstream: &Bound<'_, PyAny>,
    downstream: &Bound<'_, PyAny>,
) -> PyResult<NativeProjectGraph> {
    graph_from_dicts(GraphIndexes {
        names: Vec::new(),
        upstream: native_edges(upstream)?,
        downstream: native_edges(downstream)?,
        tags: Vec::new(),
        paths: Vec::new(),
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(native_graph_keys, module)?)?;
    module.add_function(wrap_pyfunction!(compiled_graph_keys, module)?)?;
    module.add_function(wrap_pyfunction!(lineage_graph_names, module)?)?;
    module.add_function(wrap_pyfunction!(lineage_graph_views, module)?)?;
    module.add_function(wrap_pyfunction!(project_lineage_views, module)?)?;
    module.add_function(wrap_pyfunction!(native_graph_from_views, module)?)?;
    module.add_function(wrap_pyfunction!(native_graph_from_edges, module)?)?;
    Ok(())
}
