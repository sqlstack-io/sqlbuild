//! The compiled project graph and selector resolution, held natively for Python's graph users.

use sqlbuild_python_base::positional_record;
use std::collections::HashMap;

use pyo3::prelude::{Bound, Py, PyAny, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::{pyclass, pymethods};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::graph::main::build_project_graph::build_project_graph;
use sqlbuild_analysis::graph::main::build_resources::build_resources;
use sqlbuild_analysis::graph::main::edge_keys::edge_keys;
use sqlbuild_analysis::graph::main::graph_from_indexes::graph_from_indexes;
use sqlbuild_analysis::graph::main::match_selector::match_selector;
use sqlbuild_analysis::graph::main::model_layer_count::model_layer_count;
use sqlbuild_analysis::graph::main::resolve_selector_tokens::resolve_selector_tokens;
use sqlbuild_analysis::graph::main::resolve_selectors::resolve_selectors;
use sqlbuild_analysis::graph::main::unique_id_edges::unique_id_edges;
use sqlbuild_analysis::graph::models::{
    BuildResources, EdgeDirection, GraphIndexes, GraphResource, ProjectGraph,
};

use crate::bindings::_helpers::graph::views::lineage_graph_views;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

positional_record! {
    extract;
    /// `(key, deps, tags, model folder)`.
    pub(crate) struct ResourceRow {
        pub(crate) key: ObjectKey,
        pub(crate) deps: Vec<ObjectKey>,
        pub(crate) tags: Vec<String>,
        pub(crate) folder: Option<String>,
    }
}
type Edges = Vec<(ObjectKey, Vec<ObjectKey>)>;
positional_record! {
    extract, into;
    /// `(names, upstream, downstream, tags, paths)` in Python's dict order.
    struct IndexesRow {
        names: Vec<(String, ObjectKey)>,
        upstream: Edges,
        downstream: Edges,
        tags: Vec<(String, Vec<ObjectKey>)>,
        paths: Vec<(ObjectKey, String)>,
    }
}
/// `(unique id, neighbour unique ids)` in graph order.
type UniqueIdMap = Vec<(String, Vec<String>)>;
/// `PlannerInputError`'s `(code, message, help)`.
type FailureRow = (&'static str, String, Option<String>);
/// The keys, or the `PlannerInputError` Python raises instead.
type KeysOutcome = (Option<Vec<ObjectKey>>, Option<FailureRow>);

/// A project graph: lineage edges, tag, path and name indexes, and selector resolution.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeProjectGraph {
    graph: ProjectGraph,
}

/// Static compiled project graph held natively; dict views are built only when read.
#[pyclass(name = "ProjectGraph", module = "sqlbuild._native", frozen)]
pub(crate) struct CompiledProjectGraph {
    project: Py<PyAny>,
    native: Py<NativeProjectGraph>,
    views: PyOnceLock<Py<PyAny>>,
}

fn dependency_edges(key: &ObjectKey, deps: &[ObjectKey]) -> Vec<(ObjectKey, ObjectKey)> {
    deps.iter().map(|dep| (dep.clone(), key.clone())).collect()
}

fn outcome(result: Result<Vec<ObjectKey>, SelectorError>) -> KeysOutcome {
    match result {
        Ok(keys) => (Some(keys), None),
        Err(error) => (None, Some(failure(error))),
    }
}

fn failure(error: SelectorError) -> FailureRow {
    (error.code, error.message, error.help)
}

impl NativeProjectGraph {
    /// The graph this object wraps.
    pub(crate) fn graph(&self) -> &ProjectGraph {
        &self.graph
    }

    /// Wrap a graph another native stage built, such as project assembly.
    pub(crate) fn new(graph: ProjectGraph) -> Self {
        Self { graph }
    }
}

impl CompiledProjectGraph {
    /// The graph of `project` over its native lineage graph.
    pub(crate) fn new(project: Py<PyAny>, native: Py<NativeProjectGraph>) -> Self {
        Self {
            project,
            native,
            views: PyOnceLock::new(),
        }
    }

    /// The compiled project this graph indexes.
    pub(crate) fn project<'py>(&self, py: Python<'py>) -> &Bound<'py, PyAny> {
        self.project.bind(py)
    }

    /// The native lineage graph.
    pub(crate) fn native<'py>(&self, py: Python<'py>) -> &Bound<'py, NativeProjectGraph> {
        self.native.bind(py)
    }

    fn view(&self, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        Ok(native_field(self.lineage_views(py)?.bind(py), name)?.unbind())
    }
}

#[pymethods]
impl CompiledProjectGraph {
    #[new]
    #[pyo3(signature = (*, project, native))]
    fn py_new(project: Py<PyAny>, native: Py<NativeProjectGraph>) -> Self {
        Self::new(project, native)
    }

    #[getter(project)]
    pub(crate) fn project_attribute(&self, py: Python<'_>) -> Py<PyAny> {
        self.project.clone_ref(py)
    }

    #[getter(native)]
    pub(crate) fn native_attribute(&self, py: Python<'_>) -> Py<NativeProjectGraph> {
        self.native.clone_ref(py)
    }

    /// Lineage edges and selector indexes, converted once.
    #[getter]
    pub(crate) fn lineage_views(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.views
            .get_or_try_init(py, || {
                Ok(lineage_graph_views(py, self.native.bind(py).borrow())?.unbind())
            })
            .map(|views| views.clone_ref(py))
    }

    #[getter]
    pub(crate) fn upstream_deps(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, "upstream_deps")
    }

    #[getter]
    pub(crate) fn downstream_deps(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, "downstream_deps")
    }

    #[getter]
    pub(crate) fn tag_index(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, "tag_index")
    }

    #[getter]
    pub(crate) fn path_index(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, "path_index")
    }

    #[getter]
    pub(crate) fn all_keys(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, "all_keys")
    }
}

/// The project graph indexes of these resources, in project order.
pub(crate) fn project_graph(resources: Vec<ResourceRow>) -> PyResult<NativeProjectGraph> {
    compiler_guard(|| {
        let resources: Vec<GraphResource> = resources
            .into_iter()
            .map(
                |ResourceRow {
                     key,
                     deps,
                     tags,
                     folder,
                 }| GraphResource {
                    key,
                    deps,
                    tags,
                    folder,
                },
            )
            .collect();
        Ok(NativeProjectGraph::new(build_project_graph(&resources)))
    })
}

#[pymethods]
impl NativeProjectGraph {
    /// Build the indexes from resources in project order: models, sources, seeds, functions.
    #[staticmethod]
    fn from_resources(resources: Vec<ResourceRow>) -> PyResult<Self> {
        project_graph(resources)
    }

    /// Wrap indexes a caller already built, such as the planner's.
    #[staticmethod]
    fn from_indexes(indexes: IndexesRow) -> PyResult<Self> {
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
    fn indexes(&self) -> IndexesRow {
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
    pub(crate) fn upstream(&self, key: ObjectKey) -> Vec<ObjectKey> {
        edge_keys(&self.graph, &key, EdgeDirection::Upstream).to_vec()
    }

    /// The key's direct downstream keys, sorted by `(resource type, name)`.
    fn downstream(&self, key: ObjectKey) -> Vec<ObjectKey> {
        edge_keys(&self.graph, &key, EdgeDirection::Downstream).to_vec()
    }

    /// Every selectable key, in project order with later same-name resources replacing earlier.
    fn keys(&self) -> Vec<ObjectKey> {
        self.graph
            .names
            .iter()
            .map(|(_, key)| key.clone())
            .collect()
    }

    /// `(selector name, key)` in project order.
    fn names(&self) -> Vec<(String, ObjectKey)> {
        self.graph.names.clone()
    }

    /// Every lineage edge as `(dependency, dependent)`, in upstream order.
    fn edges(&self) -> Vec<(ObjectKey, ObjectKey)> {
        self.graph
            .upstream
            .iter()
            .flat_map(|(key, deps)| dependency_edges(key, deps))
            .collect()
    }

    /// The manifest's `(parent_map, child_map)` entries as dbt-style unique ids, in graph order.
    fn unique_id_maps(
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
    fn model_layer_count(&self) -> usize {
        model_layer_count(&self.graph)
    }

    /// `--select` minus `--exclude`, plus the functions they need to build.
    fn resolve(&self, select: Vec<String>, exclude: Vec<String>) -> PyResult<KeysOutcome> {
        compiler_guard(|| Ok(outcome(resolve_selectors(&self.graph, &select, &exclude))))
    }

    /// The keys selector tokens match, without build resources.
    fn tokens(&self, selectors: Vec<String>) -> PyResult<KeysOutcome> {
        compiler_guard(|| Ok(outcome(resolve_selector_tokens(&self.graph, &selectors))))
    }

    /// The keys one `kind:value` selector names before `+` expansion.
    fn matched(&self, kind: &str, value: &str) -> PyResult<KeysOutcome> {
        compiler_guard(|| Ok(outcome(match_selector(&self.graph, kind, value))))
    }

    /// The selected keys plus the functions and seeds Python adds to build them.
    fn build_resources(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeProjectGraph>()?;
    module.add_class::<CompiledProjectGraph>()
}
