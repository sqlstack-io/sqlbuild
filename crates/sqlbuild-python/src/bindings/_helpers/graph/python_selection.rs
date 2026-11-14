//! Python node and unified SQL/Python selector resolution, and the Python node graph indexes.

use std::collections::BTreeSet;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyFrozenSet};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_analysis::python_selection::main::python_node_graph_indexes::python_node_graph_indexes as graph_indexes;
use sqlbuild_analysis::python_selection::main::resolve_python_node_excludes::resolve_python_node_excludes;
use sqlbuild_analysis::python_selection::main::resolve_python_node_selectors::resolve_python_node_selectors;
use sqlbuild_analysis::python_selection::main::resolve_python_sql_nodes::resolve_python_sql_nodes;
use sqlbuild_analysis::python_selection::main::resolve_python_sql_run_selection::resolve_python_sql_run_selection as run_selection;
use sqlbuild_analysis::python_selection::models::{
    InventoryIndexes, InventoryNode, PythonSelectionGraph, PythonSelectionNode, RunSelection,
    SqlSelectionGraph,
};

use crate::bindings::_helpers::boundary::panics::compiler_guard;
use crate::bindings::_helpers::compiled_project::job_catalog::graph_with_jobs;
use crate::bindings::_helpers::graph::project_graph::{CompiledProjectGraph, NativeProjectGraph};
use crate::bindings::_helpers::graph::python_node_graph::python_node_graph;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::ErrorId;

type NameRows = Vec<(String, Vec<String>)>;
positional_record! {
    extract;
    /// `(name, tags, POSIX relative path, upstream node names)` of one discovered node.
    struct InventoryRow {
        name: String,
        tags: Vec<String>,
        relative_path: String,
        upstream: Vec<String>,
    }
}

positional_record! {
    into;
    /// `(edges, upstream, downstream, tags, paths)` in Python's dict order.
    struct IndexesRow {
        edges: Vec<(String, String)>,
        upstream: NameRows,
        downstream: NameRows,
        tags: NameRows,
        paths: Vec<(String, String)>,
    }
}

/// The `PlannerInputError` Python raises for a selector failure.
fn planner_error(py: Python<'_>, error: SelectorError) -> PyErr {
    let raised = (|| -> PyResult<Bound<'_, PyAny>> {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("code", error.code)?;
        keywords.set_item("help", error.help)?;
        error_class(py, ErrorId::PlannerInput)?.call((error.message,), Some(&keywords))
    })();
    match raised {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}

fn strings(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

/// A `dict[str, Iterable[str]]` as rows in dict order.
fn name_rows(mapping: &Bound<'_, PyAny>) -> PyResult<NameRows> {
    let mut rows: NameRows = Vec::new();
    for item in mapping_items(mapping)?.try_iter()? {
        let item: Bound<'_, PyAny> = item?;
        rows.push((item.get_item(0)?.extract()?, strings(&item.get_item(1)?)?));
    }
    Ok(rows)
}

fn selection_node(node: &Bound<'_, PyAny>) -> PyResult<PythonSelectionNode> {
    let mut sql_deps: Vec<ObjectKey> = Vec::new();
    for reference in native_field(node, "sql_deps")?.try_iter()? {
        let reference: Bound<'_, PyAny> = reference?;
        sql_deps.push((
            native_field(&reference, "kind")?
                .getattr("value")?
                .extract()?,
            native_field(&reference, "name")?.extract()?,
        ));
    }
    Ok(PythonSelectionNode {
        name: native_field(node, "name")?.extract()?,
        kind: native_field(node, "kind")?.getattr("value")?.extract()?,
        sql_deps,
    })
}

/// A `PythonNodeGraph` as the native selection graph.
fn selection_graph(graph: &Bound<'_, PyAny>) -> PyResult<PythonSelectionGraph> {
    Ok(PythonSelectionGraph {
        nodes: native_field(graph, "nodes")?
            .try_iter()?
            .map(|node| selection_node(&node?))
            .collect::<PyResult<_>>()?,
        upstream: name_rows(&native_field(graph, "upstream_deps")?)?,
        downstream: name_rows(&native_field(graph, "downstream_deps")?)?,
        tags: name_rows(&native_field(graph, "tag_index")?)?,
        paths: native_field(graph, "path_index")?
            .call_method0("items")?
            .try_iter()?
            .map(|item| item?.extract())
            .collect::<PyResult<_>>()?,
    })
}

/// `(name, loader)` of every source in the compiled project.
fn project_sources(project: &Bound<'_, PyAny>) -> PyResult<Vec<(String, Option<String>)>> {
    native_field(project, "sources")?
        .try_iter()?
        .map(|source| {
            let source: Bound<'_, PyAny> = source?;
            Ok((
                native_field(&source, "name")?.extract()?,
                native_field(&source, "source_entry")?
                    .getattr("loader")?
                    .extract()?,
            ))
        })
        .collect()
}

/// A compiled project's sources and discovered Python nodes, which publication resolves selectors over.
pub(crate) struct ProjectSelection {
    sources: Vec<(String, Option<String>)>,
    pub(crate) nodes: PythonSelectionGraph,
}

impl ProjectSelection {
    pub(crate) fn new(
        project: &Bound<'_, PyAny>,
        discovered_inputs: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            sources: project_sources(project)?,
            nodes: selection_graph(&python_node_graph(discovered_inputs)?)?,
        })
    }

    /// The `kind:name` keys `sqb build --select` runs for these selectors.
    pub(crate) fn run_keys(
        &self,
        graph: &ProjectGraph,
        select: &[String],
        exclude: &[String],
    ) -> Result<BTreeSet<String>, String> {
        self.keys(run_selection(self.sql(graph), &self.nodes, select, exclude))
    }

    /// The `kind:name` keys these selectors name, Python checks included.
    pub(crate) fn node_keys(
        &self,
        graph: &ProjectGraph,
        select: &[String],
        exclude: &[String],
    ) -> Result<BTreeSet<String>, String> {
        self.keys(resolve_python_sql_nodes(
            self.sql(graph),
            &self.nodes,
            select,
            exclude,
        ))
    }

    /// The published key of a Python node, such as `task:export_orders`.
    pub(crate) fn node_key(&self, name: &str) -> Option<String> {
        self.nodes
            .nodes
            .iter()
            .rev()
            .find(|node| node.name == name)
            .map(|node| format!("{}:{name}", node.kind))
    }

    /// Whether a loader writes no declared source and so runs only as part of another loader's chain.
    pub(crate) fn intermediate_loader(&self, name: &str) -> bool {
        !self
            .sources
            .iter()
            .any(|(_, loader)| loader.as_deref() == Some(name))
    }

    fn sql<'graph>(&'graph self, graph: &'graph ProjectGraph) -> SqlSelectionGraph<'graph> {
        SqlSelectionGraph {
            graph,
            sources: &self.sources,
        }
    }

    fn keys(
        &self,
        selection: Result<RunSelection, SelectorError>,
    ) -> Result<BTreeSet<String>, String> {
        let selection: RunSelection = selection.map_err(|error| error.message)?;
        let mut keys: BTreeSet<String> = selection
            .sql_keys
            .into_iter()
            .map(|(kind, name)| format!("{kind}:{name}"))
            .collect();
        for name in &selection.python_node_names {
            keys.extend(self.node_key(name));
        }
        Ok(keys)
    }
}

/// The Python node names `select` minus `exclude` resolve to; every node when unselected.
pub(crate) fn python_node_names<'py>(
    graph: &Bound<'py, PyAny>,
    (select, exclude): (Vec<String>, Vec<String>),
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = graph.py();
    let native: PythonSelectionGraph = selection_graph(graph)?;
    let names: Vec<String> =
        compiler_guard(|| Ok(resolve_python_node_selectors(&native, &select, &exclude)))?
            .map_err(|error| planner_error(py, error))?;
    PyFrozenSet::new(py, names)
}

/// The Python node names `select` minus `exclude` resolve to; every node when unselected.
#[pyfunction]
#[pyo3(signature = (*, graph, select, exclude))]
fn resolve_python_node_selection<'py>(
    graph: &Bound<'py, PyAny>,
    select: Vec<String>,
    exclude: Vec<String>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    python_node_names(graph, (select, exclude))
}

/// The Python node names `sqb check` exclude tokens name, skipping tokens that match nothing.
#[pyfunction]
#[pyo3(signature = (*, graph, exclude))]
pub(crate) fn resolve_python_check_excludes<'py>(
    graph: &Bound<'py, PyAny>,
    exclude: Vec<String>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = graph.py();
    let native: PythonSelectionGraph = selection_graph(graph)?;
    let names: Vec<String> =
        compiler_guard(|| Ok(resolve_python_node_excludes(&native, &exclude)))?
            .map_err(|error| planner_error(py, error))?;
    PyFrozenSet::new(py, names)
}

/// `(sql keys, python node names)` run selectors resolve to across SQL resources and Python nodes.
#[pyfunction]
#[pyo3(signature = (*, project_graph, python_graph, select, exclude))]
pub(crate) fn resolve_python_sql_run_selection(
    project_graph: &Bound<'_, PyAny>,
    python_graph: &Bound<'_, PyAny>,
    select: Vec<String>,
    exclude: Vec<String>,
) -> PyResult<(Vec<ObjectKey>, Vec<String>)> {
    let py: Python<'_> = project_graph.py();
    let project_graph: &Bound<'_, CompiledProjectGraph> = project_graph.cast()?;
    let native: &Bound<'_, NativeProjectGraph> = project_graph.get().native(py);
    let sources: Vec<(String, Option<String>)> = project_sources(project_graph.get().project(py))?;
    let nodes: PythonSelectionGraph = selection_graph(python_graph)?;
    let node_names: Vec<&str> = nodes.nodes.iter().map(|node| node.name.as_str()).collect();
    let selectors: Vec<String> = select.iter().chain(&exclude).cloned().collect();
    let graph = graph_with_jobs(py, native.get().graph(), &selectors, &node_names)?;
    let selection: RunSelection = compiler_guard(|| {
        Ok(run_selection(
            SqlSelectionGraph {
                graph: &graph,
                sources: &sources,
            },
            &nodes,
            &select,
            &exclude,
        ))
    })?
    .map_err(|error| planner_error(py, error))?;
    Ok((selection.sql_keys, selection.python_node_names))
}

/// The dependency edges, adjacency, tag and path indexes of discovered Python nodes.
#[pyfunction]
fn python_node_graph_indexes(nodes: Vec<InventoryRow>) -> PyResult<IndexesRow> {
    compiler_guard(|| {
        let nodes: Vec<InventoryNode> = nodes
            .into_iter()
            .map(
                |InventoryRow {
                     name,
                     tags,
                     relative_path,
                     upstream,
                 }| InventoryNode {
                    name,
                    tags,
                    relative_path,
                    upstream,
                },
            )
            .collect();
        let InventoryIndexes {
            edges,
            upstream,
            downstream,
            tags,
            paths,
        } = graph_indexes(&nodes);
        Ok(IndexesRow {
            edges,
            upstream,
            downstream,
            tags,
            paths,
        })
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_python_node_selection, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_python_check_excludes, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_python_sql_run_selection, module)?)?;
    module.add_function(wrap_pyfunction!(python_node_graph_indexes, module)?)
}
