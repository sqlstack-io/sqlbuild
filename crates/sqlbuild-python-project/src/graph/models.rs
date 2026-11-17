//! Public records of graph.

use crate::graph::_helpers::project::project_graph::IndexesRow;
use crate::graph::_helpers::project::project_graph::KeysOutcome;
use crate::graph::_helpers::project::project_graph::UniqueIdMap;
use crate::graph::_helpers::project::project_graph::dependency_edges;
use crate::graph::_helpers::project::project_graph::outcome;
use crate::graph::_helpers::project::views::lineage_graph_views;
use crate::graph::_helpers::python_nodes::python_selection::project_sources;
use crate::graph::_helpers::python_nodes::python_selection::selection_graph;
use crate::graph::constants::LOADER_KIND;
use crate::graph::main::project::project_graph::project_graph;
use crate::graph::main::python_nodes::python_node_graph::python_node_graph;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::sync::PyOnceLock;
use pyo3::types::PySet;
use pyo3::types::PySetMethods;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::graph::main::build_resources::build_resources;
use sqlbuild_analysis::graph::main::edge_keys::edge_keys;
use sqlbuild_analysis::graph::main::graph_from_indexes::graph_from_indexes;
use sqlbuild_analysis::graph::main::match_selector::match_selector;
use sqlbuild_analysis::graph::main::model_layer_count::model_layer_count;
use sqlbuild_analysis::graph::main::resolve_selector_tokens::resolve_selector_tokens;
use sqlbuild_analysis::graph::main::resolve_selectors::resolve_selectors;
use sqlbuild_analysis::graph::main::unique_id_edges::unique_id_edges;
use sqlbuild_analysis::graph::models::BuildResources;
use sqlbuild_analysis::graph::models::EdgeDirection;
use sqlbuild_analysis::graph::models::GraphIndexes;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_analysis::python_selection::main::python_run_key::python_run_key;
use sqlbuild_analysis::python_selection::main::resolve_python_sql_nodes::resolve_python_sql_nodes;
use sqlbuild_analysis::python_selection::main::resolve_python_sql_run_selection::resolve_python_sql_run_selection as run_selection;
use sqlbuild_analysis::python_selection::models::PythonSelectionGraph;
use sqlbuild_analysis::python_selection::models::RunSelection;
use sqlbuild_analysis::python_selection::models::SqlSelectionGraph;
use sqlbuild_core::graph_walk::types::NodeSet;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;

/// One `CompiledObjectKey` per native pair, so shared keys convert once.
#[derive(Debug)]
pub struct KeyCache<'py> {
    pub(crate) key_type: Bound<'py, PyAny>,
    pub(crate) resource_type: Bound<'py, PyAny>,
    pub(crate) keys: RefCell<HashMap<(String, String), Py<PyAny>>>,
}

impl<'py> KeyCache<'py> {
    pub fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            key_type: record_class(py, RecordId::CompiledObjectKey)?,
            resource_type: record_class(py, RecordId::CompiledResourceType)?,
            keys: RefCell::new(HashMap::new()),
        })
    }

    pub fn py(&self) -> Python<'py> {
        self.key_type.py()
    }

    /// The `CompiledObjectKey` for one `(resource type, name)` pair.
    pub fn key(&self, (resource_type, name): &(String, String)) -> PyResult<Bound<'py, PyAny>> {
        let pair = (resource_type.clone(), name.clone());
        if let Some(existing) = self.keys.borrow().get(&pair) {
            return Ok(existing.bind(self.py()).clone());
        }
        let created: Bound<'py, PyAny> = self
            .key_type
            .call1((self.resource_type.call1((resource_type,))?, name))?;
        self.keys
            .borrow_mut()
            .insert(pair, created.clone().unbind());
        Ok(created)
    }

    /// `CompiledObjectKey`s for `pairs`, in order.
    pub fn keys(&self, pairs: &[(String, String)]) -> PyResult<Vec<Bound<'py, PyAny>>> {
        pairs.iter().map(|pair| self.key(pair)).collect()
    }
}

/// Python keys a walk visited, compared with Python hashing and equality.
#[derive(Debug)]
pub struct PyNodeSet<'py>(pub Bound<'py, PySet>);

impl<'py> NodeSet<Bound<'py, PyAny>> for PyNodeSet<'py> {
    type Error = pyo3::PyErr;

    fn insert_new(&mut self, key: &Bound<'py, PyAny>) -> Result<bool, pyo3::PyErr> {
        if self.0.contains(key)? {
            return Ok(false);
        }
        self.0.add(key)?;
        Ok(true)
    }

    fn holds(&self, key: &Bound<'py, PyAny>) -> Result<bool, pyo3::PyErr> {
        self.0.contains(key)
    }
}

/// `(key, deps, tags, model folder)`.
#[derive(Debug)]
pub struct ResourceRow {
    pub key: ObjectKey,
    pub deps: Vec<ObjectKey>,
    pub tags: Vec<String>,
    pub folder: Option<String>,
}

impl<'py> pyo3::FromPyObject<'py> for ResourceRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (key, deps, tags, folder): (ObjectKey, Vec<ObjectKey>, Vec<String>, Option<String>) =
            pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            key,
            deps,
            tags,
            folder,
        })
    }
}

/// A project graph: lineage edges, tag, path and name indexes, and selector resolution.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct NativeProjectGraph {
    pub(crate) graph: ProjectGraph,
}

/// Static compiled project graph held natively; dict views are built only when read.
#[pyclass(name = "ProjectGraph", module = "sqlbuild._native", frozen)]
pub struct CompiledProjectGraph {
    pub(crate) project: Py<PyAny>,
    pub(crate) native: Py<NativeProjectGraph>,
    pub(crate) views: PyOnceLock<Py<PyAny>>,
}

impl std::fmt::Debug for CompiledProjectGraph {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledProjectGraph")
            .finish_non_exhaustive()
    }
}

impl NativeProjectGraph {
    /// The graph this object wraps.
    pub fn graph(&self) -> &ProjectGraph {
        &self.graph
    }

    /// Wrap a graph another native stage built, such as project assembly.
    pub fn new(graph: ProjectGraph) -> Self {
        Self { graph }
    }
}

impl CompiledProjectGraph {
    /// The graph of `project` over its native lineage graph.
    pub fn new(project: Py<PyAny>, native: Py<NativeProjectGraph>) -> Self {
        Self {
            project,
            native,
            views: PyOnceLock::new(),
        }
    }

    /// The compiled project this graph indexes.
    pub fn project<'py>(&self, py: Python<'py>) -> &Bound<'py, PyAny> {
        self.project.bind(py)
    }

    /// The native lineage graph.
    pub fn native<'py>(&self, py: Python<'py>) -> &Bound<'py, NativeProjectGraph> {
        self.native.bind(py)
    }

    pub(crate) fn view(&self, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        Ok(native_field(self.lineage_views(py)?.bind(py), name)?.unbind())
    }
}

#[pymethods]
impl CompiledProjectGraph {
    #[new]
    #[pyo3(signature = (*, project, native))]
    pub(crate) fn py_new(project: Py<PyAny>, native: Py<NativeProjectGraph>) -> Self {
        Self::new(project, native)
    }

    #[getter(project)]
    pub fn project_attribute(&self, py: Python<'_>) -> Py<PyAny> {
        self.project.clone_ref(py)
    }

    #[getter(native)]
    pub fn native_attribute(&self, py: Python<'_>) -> Py<NativeProjectGraph> {
        self.native.clone_ref(py)
    }

    /// Lineage edges and selector indexes, converted once.
    #[getter]
    pub fn lineage_views(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.views
            .get_or_try_init(py, || {
                Ok(lineage_graph_views(py, self.native.bind(py).borrow())?.unbind())
            })
            .map(|views| views.clone_ref(py))
    }

    #[getter]
    pub fn upstream_deps(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, "upstream_deps")
    }

    #[getter]
    pub fn downstream_deps(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, "downstream_deps")
    }

    #[getter]
    pub fn tag_index(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, "tag_index")
    }

    #[getter]
    pub fn path_index(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, "path_index")
    }

    #[getter]
    pub fn all_keys(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, "all_keys")
    }
}

#[pymethods]
impl NativeProjectGraph {
    /// Build the indexes from resources in project order: models, sources, seeds, functions.
    #[staticmethod]
    pub(crate) fn from_resources(resources: Vec<ResourceRow>) -> PyResult<Self> {
        project_graph(resources)
    }

    /// Wrap indexes a caller already built, such as the planner's.
    #[staticmethod]
    pub(crate) fn from_indexes(indexes: IndexesRow) -> PyResult<Self> {
        compiler_guard(|| {
            let IndexesRow {
                names,
                upstream,
                downstream,
                tags,
                paths,
            } = indexes;
            Ok(Self::new(graph_from_indexes(GraphIndexes {
                names,
                upstream,
                downstream,
                tags,
                paths,
            })))
        })
    }

    /// `(names, upstream, downstream, tags, paths)` for Python's `ProjectGraph`.
    pub(crate) fn indexes(&self) -> IndexesRow {
        let graph: &ProjectGraph = &self.graph;
        IndexesRow {
            names: graph.names.clone(),
            upstream: graph.upstream.clone(),
            downstream: graph.downstream.clone(),
            tags: graph.tags.clone(),
            paths: graph.paths.clone(),
        }
    }

    /// The key's direct upstream lineage keys, in authored dependency order.
    pub fn upstream(&self, key: ObjectKey) -> Vec<ObjectKey> {
        edge_keys(&self.graph, &key, EdgeDirection::Upstream).to_vec()
    }

    /// The key's direct downstream keys, sorted by `(resource type, name)`.
    pub(crate) fn downstream(&self, key: ObjectKey) -> Vec<ObjectKey> {
        edge_keys(&self.graph, &key, EdgeDirection::Downstream).to_vec()
    }

    /// Every selectable key, in project order with later same-name resources replacing earlier.
    pub(crate) fn keys(&self) -> Vec<ObjectKey> {
        self.graph
            .names
            .iter()
            .map(|(_, key)| key.clone())
            .collect()
    }

    /// `(selector name, key)` in project order.
    pub(crate) fn names(&self) -> Vec<(String, ObjectKey)> {
        self.graph.names.clone()
    }

    /// Every lineage edge as `(dependency, dependent)`, in upstream order.
    pub(crate) fn edges(&self) -> Vec<(ObjectKey, ObjectKey)> {
        self.graph
            .upstream
            .iter()
            .flat_map(|(key, deps)| dependency_edges(key, deps))
            .collect()
    }

    /// The manifest's `(parent_map, child_map)` entries as dbt-style unique ids, in graph order.
    pub(crate) fn unique_id_maps(
        &self,
        project_name: &str,
        prefixes: HashMap<String, String>,
    ) -> (UniqueIdMap, UniqueIdMap) {
        (
            unique_id_edges(
                &self.graph,
                EdgeDirection::Upstream,
                project_name,
                &prefixes,
            ),
            unique_id_edges(
                &self.graph,
                EdgeDirection::Downstream,
                project_name,
                &prefixes,
            ),
        )
    }

    /// The compile report's execution layer count over models.
    pub(crate) fn model_layer_count(&self) -> usize {
        model_layer_count(&self.graph)
    }

    /// `--select` minus `--exclude`, plus the functions they need to build.
    pub(crate) fn resolve(
        &self,
        select: Vec<String>,
        exclude: Vec<String>,
    ) -> PyResult<KeysOutcome> {
        compiler_guard(|| Ok(outcome(resolve_selectors(&self.graph, &select, &exclude))))
    }

    /// The keys selector tokens match, without build resources.
    pub(crate) fn tokens(&self, selectors: Vec<String>) -> PyResult<KeysOutcome> {
        compiler_guard(|| Ok(outcome(resolve_selector_tokens(&self.graph, &selectors))))
    }

    /// The keys one `kind:value` selector names before `+` expansion.
    pub(crate) fn matched(&self, kind: &str, value: &str) -> PyResult<KeysOutcome> {
        compiler_guard(|| Ok(outcome(match_selector(&self.graph, kind, value))))
    }

    /// The selected keys plus the functions and seeds Python adds to build them.
    pub(crate) fn build_resources(
        &self,
        selected: Vec<ObjectKey>,
        include: (bool, bool, bool),
    ) -> PyResult<Vec<ObjectKey>> {
        compiler_guard(|| {
            let (upstream_functions, upstream_seeds, downstream_functions) = include;
            Ok(build_resources(
                &self.graph,
                &selected,
                BuildResources {
                    upstream_functions,
                    upstream_seeds,
                    downstream_functions,
                },
            ))
        })
    }
}

/// A compiled project's sources and discovered Python nodes, which publication resolves selectors over.
#[derive(Debug)]
pub struct ProjectSelection {
    pub(crate) sources: Vec<(String, Option<String>)>,
    pub nodes: PythonSelectionGraph,
}

impl ProjectSelection {
    pub fn new(project: &Bound<'_, PyAny>, discovered_inputs: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            sources: project_sources(project)?,
            nodes: selection_graph(&python_node_graph(discovered_inputs)?)?,
        })
    }

    /// The `kind:name` keys `sqb build --select` runs for these selectors.
    pub fn run_keys(
        &self,
        graph: &ProjectGraph,
        select: &[String],
        exclude: &[String],
    ) -> Result<BTreeSet<String>, String> {
        self.keys(run_selection(self.sql(graph), &self.nodes, select, exclude))
    }

    /// The `kind:name` keys these selectors name, Python checks included.
    pub fn node_keys(
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
    pub fn node_key(&self, name: &str) -> Option<String> {
        self.nodes
            .nodes
            .iter()
            .rev()
            .find(|node| node.name == name)
            .map(|node| format!("{}:{name}", node.kind))
    }

    /// Each Python node's published key and the keys it reads: its upstream Python nodes and its SQL deps.
    pub fn node_deps(&self) -> Vec<(String, BTreeSet<String>)> {
        let upstream: BTreeMap<&str, &[String]> = self
            .nodes
            .upstream
            .iter()
            .map(|(name, names)| (name.as_str(), names.as_slice()))
            .collect();
        let mut nodes: Vec<(String, BTreeSet<String>)> = Vec::new();
        for node in &self.nodes.nodes {
            let mut deps: BTreeSet<String> = BTreeSet::new();
            for name in upstream
                .get(node.name.as_str())
                .copied()
                .unwrap_or_default()
            {
                deps.extend(self.node_key(name));
            }
            for (kind, name) in &node.sql_deps {
                deps.insert(format!("{kind}:{name}"));
            }
            nodes.push((format!("{}:{}", node.kind, node.name), deps));
        }
        nodes
    }

    /// The key a selected Python node's run reports under; a loader reports as the source it writes.
    pub(crate) fn run_key(&self, name: &str) -> Option<String> {
        let node = self
            .nodes
            .nodes
            .iter()
            .rev()
            .find(|node| node.name == name)?;
        Some(python_run_key(&self.sources, (&node.kind, name)))
    }

    /// The `source:` key a loader node publishes when it writes no declared source, so its chain's other loaders can read it.
    pub fn intermediate_source(&self, kind: &str, name: &str) -> Option<String> {
        (kind == LOADER_KIND && self.intermediate_loader(name)).then(|| format!("source:{name}"))
    }

    /// Whether a loader writes no declared source and so runs only as part of another loader's chain.
    pub fn intermediate_loader(&self, name: &str) -> bool {
        !self
            .sources
            .iter()
            .any(|(_, loader)| loader.as_deref() == Some(name))
    }

    pub(crate) fn sql<'graph>(
        &'graph self,
        graph: &'graph ProjectGraph,
    ) -> SqlSelectionGraph<'graph> {
        SqlSelectionGraph {
            graph,
            sources: &self.sources,
        }
    }

    pub(crate) fn keys(
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
            keys.extend(self.run_key(name));
        }
        Ok(keys)
    }
}

/// Python's selector name, edge, tag and path mappings.
#[derive(Debug)]
pub struct GraphViewMappings<'a, 'py> {
    pub all_keys: &'a Bound<'py, PyAny>,
    pub upstream: &'a Bound<'py, PyAny>,
    pub downstream: &'a Bound<'py, PyAny>,
    pub tag_index: &'a Bound<'py, PyAny>,
    pub path_index: &'a Bound<'py, PyAny>,
}
