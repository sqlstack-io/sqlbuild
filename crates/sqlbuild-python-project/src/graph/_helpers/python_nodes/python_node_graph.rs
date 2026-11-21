//! The executable Python-node graph built from discovered project inputs.

use crate::graph::main::python_nodes::python_node_graph::python_node_graph;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple};
use sqlbuild_analysis::node_set_dependencies::main::set_dependency_metadata::set_dependency_metadata;
use sqlbuild_analysis::node_set_dependencies::models::{GraphNode, ResolvedNode, ResolvedSet};
use sqlbuild_analysis::python_selection::main::python_node_graph_indexes::python_node_graph_indexes as graph_indexes;
use sqlbuild_analysis::python_selection::models::{InventoryIndexes, InventoryNode};
use sqlbuild_host::entries::models::UserCodeHost;
use std::collections::HashMap;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::main::within_identity_pass::within_identity_pass;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::discovery::main::python_files::node_identity::node_identity;
use sqlbuild_python_front::discovery::main::python_files::provider_usages::provider_usages;

const NAME_KEY: &str = "name";

/// One discovered function collection, its node kind and whether it carries documentation.
pub(crate) const KINDS: [(&str, &str, bool); 3] = [
    ("loader_functions", "LOADER", false),
    ("asset_functions", "ASSET", true),
    ("check_functions", "CHECK", true),
];

/// Python classes and shims every node is built with.
pub(crate) struct Vocabulary<'py> {
    pub(crate) node: Bound<'py, PyAny>,
    pub(crate) kinds: Bound<'py, PyAny>,
    pub(crate) sql_ref: Bound<'py, PyAny>,
    pub(crate) providers: Bound<'py, PyDict>,
    /// Each node's resolved `job(...)` and `tag(...)` dependencies; empty for the explicit graph.
    pub(crate) sets: Vec<ResolvedNode>,
    /// Each discovered node's kind, such as `asset`, by name.
    pub(crate) node_kinds: HashMap<String, String>,
}

impl Vocabulary<'_> {
    /// Each set `name` declares, as `(set label, end node labels)` its identity records.
    fn recorded_sets(&self, name: &str) -> Vec<(String, Vec<String>)> {
        self.sets
            .iter()
            .filter(|node| node.name == name)
            .flat_map(|node| &node.sets)
            .map(|set| self.recorded_set(set))
            .collect()
    }

    /// One set's label and its end node labels.
    fn recorded_set(&self, set: &ResolvedSet) -> (String, Vec<String>) {
        let ends: Vec<String> = set.ends.iter().map(|end| self.end_label(end)).collect();
        (
            format!("{} '{}'", set.dependency.kind, set.dependency.name),
            ends,
        )
    }

    /// `model 'orders'` or `asset 'export_orders'`.
    fn end_label(&self, end: &GraphNode) -> String {
        match end {
            GraphNode::Sql((kind, name)) => format!("{kind} '{name}'"),
            GraphNode::Python(name) => format!(
                "{} '{name}'",
                self.node_kinds.get(name).map_or("node", String::as_str)
            ),
        }
    }

    /// The end nodes of every set dependency `name` declares.
    fn set_ends<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a GraphNode> + 'a {
        self.sets
            .iter()
            .filter(move |node| node.name == name)
            .flat_map(|node| &node.sets)
            .flat_map(|set| &set.ends)
    }
}

/// A `SqlResourceRef` to each SQL end node of `name`'s sets that it does not already declare.
fn set_sql_refs<'py>(
    vocabulary: &Vocabulary<'py>,
    name: &str,
    mut declared: Vec<(String, String)>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = vocabulary.sql_ref.py();
    let kinds: Bound<'py, PyAny> = host_entry(py, UserCodeHost::SqlResourceRefKind)?;
    let mut references: Vec<Bound<'py, PyAny>> = Vec::new();
    for end in vocabulary.set_ends(name) {
        let GraphNode::Sql(key) = end else {
            continue;
        };
        if declared.contains(key) {
            continue;
        }
        declared.push(key.clone());
        references.push(call_with(
            &vocabulary.sql_ref,
            &[
                ("kind", native_field(&kinds, &key.0.to_uppercase())?),
                ("name", PyString::new(py, &key.1).into_any()),
            ],
        )?);
    }
    Ok(references)
}

/// `identity` with `recorded` sets in its metadata, so a later plan can list wait changes.
fn with_recorded_sets<'py>(
    identity: Bound<'py, PyAny>,
    recorded: &[(String, Vec<String>)],
) -> PyResult<Bound<'py, PyAny>> {
    if recorded.is_empty() || identity.is_none() {
        return Ok(identity);
    }
    let metadata: String = native_field(&identity, "metadata_json")?.extract()?;
    let updated: String = set_dependency_metadata(&metadata, recorded);
    replaced(
        &identity,
        &[(
            "metadata_json",
            PyString::new(identity.py(), &updated).into_any(),
        )],
    )
}

/// `(kind, name)` of a `SqlResourceRef`.
fn sql_ref_key(reference: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    Ok((
        native_field(&native_field(reference, "kind")?, "value")?.extract()?,
        native_field(reference, "name")?.extract()?,
    ))
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
                ],
            )?,
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
    let name: String = get(NAME_KEY)?.extract()?;
    identity = with_recorded_sets(identity, &vocabulary.recorded_sets(&name))?;
    let depends_on: Bound<'py, PyAny> = get("depends_on")?;
    let mut sql_deps: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut declared: Vec<(String, String)> = Vec::new();
    for dependency in depends_on.try_iter()? {
        let dependency: Bound<'py, PyAny> = dependency?;
        if dependency.is_instance(&vocabulary.sql_ref)? {
            declared.push(sql_ref_key(&dependency)?);
            sql_deps.push(dependency);
        }
    }
    sql_deps.extend(set_sql_refs(vocabulary, &name, declared)?);
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
    vocabulary: &Vocabulary<'_>,
) -> PyResult<Vec<Vec<String>>> {
    let sql_ref: &Bound<'_, PyAny> = &vocabulary.sql_ref;
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
            let name: String = native_field(node, "name")?.extract()?;
            for end in vocabulary.set_ends(&name) {
                if let GraphNode::Python(upstream) = end
                    && !names.contains(upstream)
                {
                    names.push(upstream.clone());
                }
            }
            Ok(names)
        })
        .collect()
}

/// Each discovered node's lowercase kind, by name.
fn node_kinds(discovered_inputs: &Bound<'_, PyAny>) -> PyResult<HashMap<String, String>> {
    let mut kinds: HashMap<String, String> = HashMap::new();
    for (collection, kind, _) in KINDS {
        for discovered in native_field(discovered_inputs, collection)?.try_iter()? {
            kinds.insert(
                native_field(&discovered?, NAME_KEY)?.extract()?,
                kind.to_lowercase(),
            );
        }
    }
    Ok(kinds)
}

/// The Python-node graph of only the dependencies the nodes name one by one.
pub(crate) fn explicit_python_node_graph<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    graph_with_sets(discovered_inputs, Vec::new())
}

/// The executable Python-node graph with each of `sets` an edge to its end nodes.
pub(crate) fn graph_with_sets<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    sets: Vec<ResolvedNode>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered_inputs.py();
    let providers: Bound<'py, PyDict> = PyDict::new(py);
    for provider in native_field(discovered_inputs, "providers")?.try_iter()? {
        let provider: Bound<'py, PyAny> = provider?;
        providers.set_item(native_field(&provider, "name")?, provider)?;
    }
    let vocabulary: Vocabulary<'py> = Vocabulary {
        node: record_class(py, RecordId::DiscoveredPythonNode)?,
        kinds: host_entry(py, UserCodeHost::PythonNodeKind)?,
        sql_ref: host_entry(py, UserCodeHost::SqlResourceRef)?,
        providers,
        node_kinds: if sets.is_empty() {
            HashMap::new()
        } else {
            node_kinds(discovered_inputs)?
        },
        sets,
    };
    let mut nodes: Vec<Bound<'py, PyAny>> = Vec::new();
    within_identity_pass(py, || {
        for (collection, kind, documented) in KINDS {
            for discovered in native_field(discovered_inputs, collection)?.try_iter()? {
                nodes.push(python_node(&vocabulary, (kind, documented), &discovered?)?);
            }
        }
        Ok(())
    })?;
    let inventory: Vec<InventoryNode> = nodes
        .iter()
        .zip(upstream_names(py, &nodes, &vocabulary)?)
        .map(|(node, upstream)| inventory_node(node, upstream))
        .collect::<PyResult<_>>()?;
    graph_record(py, &nodes, graph_indexes(&inventory))
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
