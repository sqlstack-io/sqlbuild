//! The executable Python-node graph built from discovered project inputs.

use crate::graph::main::python_nodes::python_node_graph::python_node_graph;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use sqlbuild_analysis::python_selection::models::{InventoryIndexes, InventoryNode};
use sqlbuild_host::entries::models::UserCodeHost;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::discovery::main::python_files::node_identity::node_identity;
use sqlbuild_python_front::discovery::main::python_files::provider_usages::provider_usages;

const NAME_KEY: &str = "name";

/// One discovered function collection, its node kind and whether it carries documentation.
pub(crate) const KINDS: [(&str, &str, bool); 4] = [
    ("loader_functions", "LOADER", false),
    ("task_functions", "TASK", true),
    ("asset_functions", "ASSET", true),
    ("check_functions", "CHECK", true),
];

/// Python classes and shims every node is built with.
pub(crate) struct Vocabulary<'py> {
    pub(crate) node: Bound<'py, PyAny>,
    pub(crate) kinds: Bound<'py, PyAny>,
    pub(crate) sql_ref: Bound<'py, PyAny>,
    pub(crate) providers: Bound<'py, PyDict>,
}

fn attribute_value<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    native_field(value, name)
}

/// The kind's metadata field and record of one discovered function.
fn kind_metadata<'py>(
    py: Python<'py>,
    kind: &str,
    discovered: &Bound<'py, PyAny>,
) -> PyResult<(&'static str, Bound<'py, PyAny>)> {
    let get = |name: &str| attribute_value(discovered, name);
    let metadata = |class: RecordId, fields: &[&str]| -> PyResult<Bound<'py, PyAny>> {
        let pairs: Vec<(&str, Bound<'py, PyAny>)> = fields
            .iter()
            .map(|field| Ok((*field, get(field)?)))
            .collect::<PyResult<_>>()?;
        new_record_with(py, class, &pairs)
    };
    Ok(match kind {
        "LOADER" => (
            "loader",
            metadata(
                RecordId::DiscoveredPythonLoaderMetadata,
                &[
                    "destination",
                    "write_strategy",
                    "cursor_column",
                    "unique_key",
                    "columns",
                    "contract",
                    "connection_mode",
                ],
            )?,
        ),
        "TASK" => (
            "task",
            metadata(RecordId::DiscoveredPythonTaskMetadata, &["retry"])?,
        ),
        "ASSET" => (
            "asset",
            metadata(
                RecordId::DiscoveredPythonAssetMetadata,
                &["columns", "column_lineage", "retry"],
            )?,
        ),
        _ => (
            "check",
            metadata(RecordId::DiscoveredPythonCheckMetadata, &["severity"])?,
        ),
    })
}

/// One `DiscoveredPythonNode` with its identity and provider usages read by the user shims.
pub(crate) fn python_node<'py>(
    vocabulary: &Vocabulary<'py>,
    (kind, documented): (&str, bool),
    discovered: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let get = |name: &str| attribute_value(discovered, name);
    let (metadata_field, metadata) = kind_metadata(py, kind, discovered)?;
    let kind_member: Bound<'py, PyAny> = native_field(&vocabulary.kinds, kind)?;
    let usages: Bound<'py, PyAny> = provider_usages(&get("function")?, &vocabulary.providers)?;
    let mut identity: Bound<'py, PyAny> = get("identity")?;
    if identity.is_none() {
        identity = node_identity(py, (kind, documented), &get)?;
    }
    let depends_on: Bound<'py, PyAny> = get("depends_on")?;
    let mut sql_deps: Vec<Bound<'py, PyAny>> = Vec::new();
    for dependency in depends_on.try_iter()? {
        let dependency: Bound<'py, PyAny> = dependency?;
        if dependency.is_instance(&vocabulary.sql_ref)? {
            sql_deps.push(dependency);
        }
    }
    let mut pairs: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("kind", kind_member),
        ("file_path", get("file_path")?),
        ("relative_path", get("relative_path")?),
        ("name", get("name")?),
        ("function", get("function")?),
        ("depends_on", depends_on),
        ("sql_deps", PyTuple::new(py, sql_deps)?.into_any()),
        ("provider_usages", usages),
        ("identity", identity),
        ("description", get("description")?),
        (metadata_field, metadata),
    ];
    if documented {
        for field in ["tags", "group", "meta"] {
            pairs.push((field, get(field)?));
        }
    }
    call_with(&vocabulary.node, &pairs)
}

/// Each node's upstream Python-node names, resolving decorated dependencies through the shim.
pub(crate) fn upstream_names(
    py: Python<'_>,
    nodes: &[Bound<'_, PyAny>],
    sql_ref: &Bound<'_, PyAny>,
) -> PyResult<Vec<Vec<String>>> {
    let by_key: Bound<'_, PyDict> = PyDict::new(py);
    for node in nodes {
        by_key.set_item(native_field(node, "function")?, native_field(node, "name")?)?;
    }
    for node in nodes {
        let name: Bound<'_, PyAny> = native_field(node, "name")?;
        by_key.set_item(
            PyTuple::new(py, [PyString::new(py, NAME_KEY).into_any(), name.clone()])?,
            name,
        )?;
    }
    let dependency_key: Bound<'_, PyAny> = host_entry(py, UserCodeHost::PythonNodeDependencyKey)?;
    nodes
        .iter()
        .map(|node| {
            let mut names: Vec<String> = Vec::new();
            for dependency in native_field(node, "depends_on")?.try_iter()? {
                let dependency: Bound<'_, PyAny> = dependency?;
                if dependency.is_instance(sql_ref)? {
                    continue;
                }
                let key: Bound<'_, PyAny> = dependency_key.call1((dependency,))?;
                if let Some(upstream) = by_key.get_item(key)? {
                    names.push(upstream.extract()?);
                }
            }
            Ok(names)
        })
        .collect()
}

pub(crate) fn inventory_node(
    node: &Bound<'_, PyAny>,
    upstream: Vec<String>,
) -> PyResult<InventoryNode> {
    Ok(InventoryNode {
        name: native_field(node, "name")?.extract()?,
        tags: native_field(node, "tags")?.extract()?,
        relative_path: native_field(node, "relative_path")?
            .call_method0("as_posix")?
            .extract()?,
        upstream,
    })
}

type NameRows = Vec<(String, Vec<String>)>;

pub(crate) fn graph_record<'py>(
    py: Python<'py>,
    nodes: &[Bound<'py, PyAny>],
    indexes: InventoryIndexes,
) -> PyResult<Bound<'py, PyAny>> {
    let InventoryIndexes {
        edges,
        upstream,
        downstream,
        tags,
        paths,
    } = indexes;
    let dependency_edges: Vec<Bound<'py, PyAny>> = edges
        .into_iter()
        .map(|(upstream_name, downstream_name)| {
            new_record_with(
                py,
                RecordId::PythonNodeDependencyEdge,
                &[
                    (
                        "upstream_name",
                        PyString::new(py, &upstream_name).into_any(),
                    ),
                    (
                        "downstream_name",
                        PyString::new(py, &downstream_name).into_any(),
                    ),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    let adjacency = |rows: NameRows| -> PyResult<Bound<'py, PyDict>> {
        let mapping: Bound<'py, PyDict> = PyDict::new(py);
        for (name, names) in rows {
            mapping.set_item(name, PyTuple::new(py, names)?)?;
        }
        Ok(mapping)
    };
    let tag_index: Bound<'py, PyDict> = PyDict::new(py);
    for (tag, names) in tags {
        tag_index.set_item(tag, PyFrozenSet::new(py, names)?)?;
    }
    let path_index: Bound<'py, PyDict> = PyDict::new(py);
    for (name, path) in paths {
        path_index.set_item(name, path)?;
    }
    let by_name: Bound<'py, PyDict> = PyDict::new(py);
    let by_selector: Bound<'py, PyDict> = PyDict::new(py);
    for node in nodes {
        let name: Bound<'py, PyAny> = native_field(node, "name")?;
        let kind: String = native_field(node, "kind")?.getattr("value")?.extract()?;
        by_name.set_item(&name, node)?;
        by_selector.set_item(format!("{kind}:{}", name.str()?), node)?;
    }
    new_record_with(
        py,
        RecordId::PythonNodeGraph,
        &[
            ("nodes", PyTuple::new(py, nodes)?.into_any()),
            (
                "dependency_edges",
                PyTuple::new(py, dependency_edges)?.into_any(),
            ),
            ("upstream_deps", adjacency(upstream)?.into_any()),
            ("downstream_deps", adjacency(downstream)?.into_any()),
            ("tag_index", tag_index.into_any()),
            ("path_index", path_index.into_any()),
            ("nodes_by_name", by_name.into_any()),
            ("nodes_by_typed_selector", by_selector.into_any()),
        ],
    )
}

/// Build the internal executable Python-node graph from discovered project inputs.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_python_node_graph<'py>(
    options: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("build_python_node_graph", options)?;
    python_node_graph(&request.get("discovered_inputs")?)
}
