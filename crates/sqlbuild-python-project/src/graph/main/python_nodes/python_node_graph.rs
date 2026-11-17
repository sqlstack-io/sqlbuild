//! The internal executable Python-node graph of discovered project inputs.

use crate::graph::_helpers::python_nodes::python_node_graph::KINDS;
use crate::graph::_helpers::python_nodes::python_node_graph::Vocabulary;
use crate::graph::_helpers::python_nodes::python_node_graph::graph_record;
use crate::graph::_helpers::python_nodes::python_node_graph::inventory_node;
use crate::graph::_helpers::python_nodes::python_node_graph::python_node;
use crate::graph::_helpers::python_nodes::python_node_graph::upstream_names;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::python_selection::main::python_node_graph_indexes::python_node_graph_indexes as graph_indexes;
use sqlbuild_analysis::python_selection::models::InventoryNode;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::main::within_identity_pass::within_identity_pass;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// The internal executable Python-node graph of discovered project inputs.
pub fn python_node_graph<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
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
        .zip(upstream_names(py, &nodes, &vocabulary.sql_ref)?)
        .map(|(node, upstream)| inventory_node(node, upstream))
        .collect::<PyResult<_>>()?;
    graph_record(py, &nodes, graph_indexes(&inventory))
}
