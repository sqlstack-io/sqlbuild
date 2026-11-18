//! Python nodes' `job(...)` and `tag(...)` dependencies, resolved once a compile has the SQL graph.

use std::collections::BTreeMap;
use std::sync::PoisonError;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::graph::models::JobSelection;
use sqlbuild_analysis::node_set_dependencies::main::resolve_node_set_dependencies::resolve_node_set_dependencies;
use sqlbuild_analysis::node_set_dependencies::models::{DependantNode, NodeSetDependency};
use sqlbuild_analysis::python_selection::models::{PythonSelectionGraph, SqlSelectionGraph};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::boundary::panics::compiler_guard;
use crate::bindings::_helpers::compiled_project::job_catalog::declared_jobs;
use crate::bindings::_helpers::graph::project_graph::NativeProjectGraph;
use crate::bindings::_helpers::graph::python_node_graph::explicit_python_node_graph;
use crate::bindings::_helpers::graph::python_selection::{
    allows_round_trips, project_sources, selection_graph,
};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::models::NativeDiscoverySession;
use crate::bindings::types::NodeSetOutcome;

/// Discovered function collections and the node kind each holds.
const COLLECTIONS: [(&str, &str); 4] = [
    ("loader_functions", "loader"),
    ("task_functions", "task"),
    ("asset_functions", "asset"),
    ("check_functions", "check"),
];

/// Each discovered Python node declaring a `job(...)` or `tag(...)` dependency.
fn dependants(discovered: &Bound<'_, PyAny>) -> PyResult<Vec<DependantNode>> {
    let node_set: Bound<'_, PyAny> = host::entry(discovered.py(), UserCodeHost::NodeSetRef)?;
    let mut found: Vec<DependantNode> = Vec::new();
    for (collection, kind) in COLLECTIONS {
        for function in native_field(discovered, collection)?.try_iter()? {
            let function: Bound<'_, PyAny> = function?;
            let mut sets: Vec<NodeSetDependency> = Vec::new();
            for dependency in native_field(&function, "depends_on")?.try_iter()? {
                let dependency: Bound<'_, PyAny> = dependency?;
                if dependency.is_instance(&node_set)? {
                    sets.push(NodeSetDependency {
                        kind: native_field(&native_field(&dependency, "kind")?, "value")?
                            .extract()?,
                        name: native_field(&dependency, "name")?.extract()?,
                        exclude: native_field(&dependency, "exclude")?.extract()?,
                    });
                }
            }
            if !sets.is_empty() {
                found.push(DependantNode {
                    name: native_field(&function, "name")?.extract()?,
                    kind: kind.to_owned(),
                    sets,
                });
            }
        }
    }
    Ok(found)
}

/// The discovery session the inputs were read in, which holds their resolved set dependencies.
fn session<'py>(
    discovered: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, NativeDiscoverySession>>> {
    let session: Bound<'py, PyAny> = native_field(discovered, "native_session")?;
    if session.is_none() {
        return Ok(None);
    }
    Ok(Some(session.cast_into::<NativeDiscoverySession>()?))
}

/// The resolved set dependencies the latest compile of `discovered` stored, if any.
pub(crate) fn node_set_outcome(discovered: &Bound<'_, PyAny>) -> PyResult<Option<NodeSetOutcome>> {
    Ok(session(discovered)?.and_then(|session| {
        session
            .get()
            .node_sets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }))
}

/// Resolve and keep the set dependencies of `discovered` over `project`'s graph, errors included.
pub(crate) fn resolve_node_sets(
    discovered: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
) -> PyResult<Option<NodeSetOutcome>> {
    let found: Vec<DependantNode> = dependants(discovered)?;
    let outcome: Option<NodeSetOutcome> = if found.is_empty() {
        None
    } else {
        let native: Bound<'_, NativeProjectGraph> =
            native_field(project, "lineage_graph")?.cast_into()?;
        let sources: Vec<(String, Option<String>)> = project_sources(project)?;
        let python: PythonSelectionGraph =
            selection_graph(&explicit_python_node_graph(discovered)?)?;
        let project_dir: Bound<'_, PyAny> = native_field(discovered, "project_dir")?;
        let catalog: BTreeMap<String, JobSelection> = if project_dir.is_none() {
            BTreeMap::new()
        } else {
            declared_jobs(&project_dir)?
        };
        let allow_warehouse_round_trips: bool = allows_round_trips(project)?;
        Some(compiler_guard(|| {
            Ok(resolve_node_set_dependencies(
                SqlSelectionGraph {
                    graph: native.get().graph(),
                    sources: &sources,
                    allow_warehouse_round_trips,
                },
                &python,
                &found,
                &catalog,
            ))
        })?)
    };
    if let Some(session) = session(discovered)? {
        *session
            .get()
            .node_sets
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = outcome.clone();
    }
    Ok(outcome)
}
