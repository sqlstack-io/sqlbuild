//! One publication indexed for planning: its selector graphs are built once and shared by every command planned over it.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use sqlbuild_analysis::definition_changes::constants::PYTHON_NODE_KINDS;
use sqlbuild_analysis::graph::main::attach_jobs::attach_jobs;
use sqlbuild_analysis::graph::main::build_project_graph::build_project_graph;
use sqlbuild_analysis::graph::models::{GraphResource, JobSelection, ProjectGraph};
use sqlbuild_analysis::planner_selection::models::UnitTestSelectorNames;
use sqlbuild_analysis::python_selection::main::published_python_graph::published_python_graph;
use sqlbuild_analysis::python_selection::models::{PublishedPythonNode, PythonSelectionGraph};
use sqlbuild_orchestration::runs::models::PublishedJobs;
use sqlbuild_orchestration::runs::models::{PlannerResource, SelectionVocabulary};

use crate::adhoc::constants::SQL_TEST_KIND;

/// A publication's resources and jobs with the indexes selector resolution reads.
#[derive(Debug)]
pub(crate) struct IndexedPublication {
    pub(crate) resources: Vec<PlannerResource>,
    pub(crate) jobs: PublishedJobs,
    /// SQL unit test names, and every other resource and job name, for splitting `test:` selectors.
    pub(crate) names: UnitTestSelectorNames,
    /// `(source name, loader)` for every published source.
    pub(crate) sources: Vec<(String, Option<String>)>,
    /// The SQL resources' graph without jobs; most commands never name one.
    pub(crate) graph: ProjectGraph,
    /// [`Self::graph`] with jobs attached, without and with their Python nodes, built when a command first names a job.
    with_jobs: [OnceLock<ProjectGraph>; 2],
    /// Python nodes, for commands that run them.
    pub(crate) python: PythonIndex,
    /// No Python nodes, for commands that do not run them.
    pub(crate) no_python: PythonIndex,
    /// The values each selector type takes, built when first asked for.
    pub(crate) vocabulary: OnceLock<SelectionVocabulary>,
}

/// The Python nodes a command may select, as the unified selector reads them.
#[derive(Debug)]
pub(crate) struct PythonIndex {
    /// Node names, which a bare selector may name instead of a job.
    pub(crate) names: Vec<String>,
    pub(crate) graph: PythonSelectionGraph,
    /// Node name to kind, such as `task`.
    pub(crate) kinds: BTreeMap<String, String>,
}

impl IndexedPublication {
    pub(crate) fn new(resources: Vec<PlannerResource>, jobs: PublishedJobs) -> Self {
        let (tests, others): (Vec<&PlannerResource>, Vec<&PlannerResource>) =
            resources.iter().partition(|resource| is_sql_test(resource));
        let names = UnitTestSelectorNames {
            tests: tests
                .iter()
                .map(|resource| name_of(&resource.key).to_owned())
                .collect(),
            resources: others
                .iter()
                .map(|resource| name_of(&resource.key).to_owned())
                .chain(jobs.selections.keys().cloned())
                .collect(),
        };
        let (python, sql): (Vec<&PlannerResource>, Vec<&PlannerResource>) = others
            .into_iter()
            .partition(|resource| python_kind(&resource.key).is_some());
        let graph_resources: Vec<GraphResource> = sql
            .iter()
            .filter_map(|resource| graph_resource(resource))
            .collect();
        let sources: Vec<(String, Option<String>)> = sql
            .iter()
            .filter(|resource| resource.key.starts_with("source:"))
            .map(|resource| (name_of(&resource.key).to_owned(), resource.loader.clone()))
            .collect();
        let nodes: Vec<PublishedPythonNode> = python
            .iter()
            .filter_map(|resource| published_python_node(resource))
            .collect();
        let python = PythonIndex {
            names: python
                .iter()
                .map(|resource| name_of(&resource.key).to_owned())
                .collect(),
            graph: published_python_graph(&nodes),
            kinds: nodes
                .iter()
                .map(|node| (node.name.clone(), node.kind.clone()))
                .collect(),
        };
        Self {
            graph: build_project_graph(&graph_resources),
            with_jobs: [OnceLock::new(), OnceLock::new()],
            vocabulary: OnceLock::new(),
            no_python: PythonIndex {
                names: Vec::new(),
                graph: published_python_graph(&[]),
                kinds: BTreeMap::new(),
            },
            python,
            sources,
            names,
            resources,
            jobs,
        }
    }

    /// The SQL graph with jobs attached, their Python nodes kept only when the command runs Python.
    pub(crate) fn graph_with_jobs(&self, include_python: bool) -> &ProjectGraph {
        self.with_jobs[usize::from(include_python)].get_or_init(|| {
            attach_jobs(
                self.graph.clone(),
                published_jobs(&self.jobs, include_python),
            )
        })
    }

    /// The Python nodes a command sees: all of them when it runs Python, else none.
    pub(crate) const fn python(&self, include_python: bool) -> &PythonIndex {
        if include_python {
            &self.python
        } else {
            &self.no_python
        }
    }
}

pub(crate) fn is_sql_test(resource: &PlannerResource) -> bool {
    resource.key.split_once(':').map(|(kind, _)| kind) == Some(SQL_TEST_KIND)
}

/// The Python node kind of a published key, such as `task` for `task:export_orders`.
pub(crate) fn python_kind(key: &str) -> Option<&'static str> {
    let (kind, _) = key.split_once(':')?;
    PYTHON_NODE_KINDS.into_iter().find(|python| *python == kind)
}

pub(crate) fn name_of(key: &str) -> &str {
    key.split_once(':').map_or(key, |(_, name)| name)
}

fn published_python_node(resource: &PlannerResource) -> Option<PublishedPythonNode> {
    Some(PublishedPythonNode {
        name: name_of(&resource.key).to_owned(),
        kind: python_kind(&resource.key)?.to_owned(),
        tags: resource.tags.clone(),
        folder: resource.folder.clone().unwrap_or_else(|| ".".to_owned()),
        deps: resource
            .deps
            .iter()
            .filter_map(|dep| object_key(dep))
            .collect(),
    })
}

/// Published jobs as already-resolved graph selections.
fn published_jobs(jobs: &PublishedJobs, include_python: bool) -> BTreeMap<String, JobSelection> {
    let mut selections: BTreeMap<String, JobSelection> = BTreeMap::new();
    for (name, keys) in &jobs.selections {
        let keys: Vec<(String, String)> = keys
            .iter()
            .filter(|key| include_python || python_kind(key).is_none())
            .filter_map(|key| object_key(key))
            .collect();
        selections.insert(name.clone(), JobSelection::Resolved { keys });
    }
    selections
}

/// A published node as the selector graph sees it; keys without a kind are not selectable.
fn graph_resource(resource: &PlannerResource) -> Option<GraphResource> {
    Some(GraphResource {
        key: object_key(&resource.key)?,
        deps: resource
            .deps
            .iter()
            .map(String::as_str)
            .filter_map(object_key)
            .collect(),
        tags: resource.tags.clone(),
        folder: resource.folder.clone(),
    })
}

fn object_key(key: &str) -> Option<(String, String)> {
    key.split_once(':')
        .map(|(kind, name)| (kind.to_owned(), name.to_owned()))
}
