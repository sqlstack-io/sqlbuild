//! Python node and unified SQL/Python selector resolution, and the Python node graph indexes.

use crate::graph::main::python_nodes::python_node_names::python_node_names;
use sqlbuild_python_base::positional_record;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyFrozenSet;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_analysis::python_selection::main::attach_python_jobs::attach_python_jobs;
use sqlbuild_analysis::python_selection::main::python_node_graph_indexes::python_node_graph_indexes as graph_indexes;
use sqlbuild_analysis::python_selection::main::resolve_python_sql_run_selection::resolve_python_sql_run_selection as run_selection;
use sqlbuild_analysis::python_selection::models::{
    InventoryIndexes, InventoryNode, PythonSelectionGraph, PythonSelectionNode, RunSelection,
    SqlSelectionGraph,
};

use crate::compiled_project::main::needed_jobs::needed_jobs;
use crate::graph::models::{CompiledProjectGraph, NativeProjectGraph, PythonAwareSelection};
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::ErrorId;

type NameRows = Vec<(String, Vec<String>)>;
positional_record! {
    extract;
    /// `(name, tags, POSIX relative path, upstream node names)` of one discovered node.
    pub(crate) struct InventoryRow {
        name: String,
        tags: Vec<String>,
        relative_path: String,
        upstream: Vec<String>,
    }
}

positional_record! {
    into;
    /// `(edges, upstream, downstream, tags, paths)` in Python's dict order.
    pub(crate) struct IndexesRow {
        edges: Vec<(String, String)>,
        upstream: NameRows,
        downstream: NameRows,
        tags: NameRows,
        paths: Vec<(String, String)>,
    }
}

/// The `PlannerInputError` Python raises for a selector failure.
pub(crate) fn planner_error(py: Python<'_>, error: SelectorError) -> PyErr {
    new_error(
        py,
        ErrorId::PlannerInput,
        &error.message,
        (Some(error.code), error.help.as_deref()),
    )
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

/// The `(kind, name)` keys of the SQL resources a Python node reads, in authored order.
pub(crate) fn sql_deps(node: &Bound<'_, PyAny>) -> PyResult<Vec<ObjectKey>> {
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
    Ok(sql_deps)
}

fn selection_node(node: &Bound<'_, PyAny>) -> PyResult<PythonSelectionNode> {
    Ok(PythonSelectionNode {
        name: native_field(node, "name")?.extract()?,
        kind: native_field(node, "kind")?.getattr("value")?.extract()?,
        sql_deps: sql_deps(node)?,
    })
}

/// A `PythonNodeGraph` as the native selection graph.
pub(crate) fn selection_graph(graph: &Bound<'_, PyAny>) -> PyResult<PythonSelectionGraph> {
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
pub(crate) fn project_sources(
    project: &Bound<'_, PyAny>,
) -> PyResult<Vec<(String, Option<String>)>> {
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

/// `[settings] allow_warehouse_round_trips` of a compiled project.
pub(crate) fn allows_round_trips(project: &Bound<'_, PyAny>) -> PyResult<bool> {
    native_field(
        &native_field(project, "settings")?,
        "allow_warehouse_round_trips",
    )?
    .extract()
}

/// The Python node names `select` minus `exclude` resolve to; every node when unselected.
#[pyfunction]
#[pyo3(signature = (*, graph, select, exclude))]
pub(crate) fn resolve_python_node_selection<'py>(
    graph: &Bound<'py, PyAny>,
    select: Vec<String>,
    exclude: Vec<String>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    python_node_names(graph, (select, exclude))
}

impl<'g> PythonAwareSelection<'g> {
    /// The graph of `native` with the jobs `selectors` need attached over `python_graph`.
    pub fn new(
        (project_graph, native): (&Bound<'_, CompiledProjectGraph>, &'g ProjectGraph),
        python_graph: &Bound<'_, PyAny>,
        selectors: &[String],
    ) -> PyResult<Self> {
        let py: Python<'_> = project_graph.py();
        let sources: Vec<(String, Option<String>)> =
            project_sources(project_graph.get().project(py))?;
        let allow_warehouse_round_trips: bool =
            allows_round_trips(project_graph.get().project(py))?;
        let nodes: PythonSelectionGraph = selection_graph(python_graph)?;
        let node_names: Vec<&str> = nodes.nodes.iter().map(|node| node.name.as_str()).collect();
        let graph: Cow<'g, ProjectGraph> = match needed_jobs(py, native, selectors, &node_names)? {
            Some(catalog) => Cow::Owned(attach_python_jobs(
                SqlSelectionGraph {
                    graph: native,
                    sources: &sources,
                    allow_warehouse_round_trips,
                },
                &nodes,
                catalog,
            )),
            None => Cow::Borrowed(native),
        };
        Ok(Self {
            graph,
            sources,
            allow_warehouse_round_trips,
            nodes,
        })
    }

    /// The graph with the needed jobs attached.
    pub fn graph(&self) -> &ProjectGraph {
        &self.graph
    }

    /// The SQL keys and Python node `(kind, name)` keys `select` minus `exclude` resolve to.
    pub fn resolve_keys(
        &self,
        select: &[String],
        exclude: &[String],
    ) -> Result<BTreeSet<ObjectKey>, SelectorError> {
        let RunSelection {
            sql_keys,
            python_node_names,
        } = self.resolve(select, exclude)?;
        let kinds: BTreeMap<&str, &str> = self
            .nodes
            .nodes
            .iter()
            .map(|node| (node.name.as_str(), node.kind.as_str()))
            .collect();
        let mut keys: BTreeSet<ObjectKey> = sql_keys.into_iter().collect();
        for name in python_node_names {
            if let Some(kind) = kinds.get(name.as_str()) {
                keys.insert(((*kind).to_owned(), name));
            }
        }
        Ok(keys)
    }

    /// The SQL keys and Python node names `select` minus `exclude` resolve to.
    pub fn resolve(
        &self,
        select: &[String],
        exclude: &[String],
    ) -> Result<RunSelection, SelectorError> {
        let sql = SqlSelectionGraph {
            graph: &self.graph,
            sources: &self.sources,
            allow_warehouse_round_trips: self.allow_warehouse_round_trips,
        };
        run_selection(sql, &self.nodes, select, exclude)
    }
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
    let selectors: Vec<String> = select.iter().chain(&exclude).cloned().collect();
    let resolution = PythonAwareSelection::new(
        (project_graph, native.get().graph()),
        python_graph,
        &selectors,
    )?;
    let selection: RunSelection = compiler_guard(|| Ok(resolution.resolve(&select, &exclude)))?
        .map_err(|error| planner_error(py, error))?;
    Ok((selection.sql_keys, selection.python_node_names))
}

/// The dependency edges, adjacency, tag and path indexes of discovered Python nodes.
#[pyfunction]
pub(crate) fn python_node_graph_indexes(nodes: Vec<InventoryRow>) -> PyResult<IndexesRow> {
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
